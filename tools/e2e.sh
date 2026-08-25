#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Glovrex
# what : the E2E audit. Clones this repository from git and builds + tests only what the
#        clone holds, so a green working tree can never be mistaken for a green
#        repository. Source: req/05_REQ0.01_SCAFFOLD_REQDEF_2026-08-06.md §3 step 3
#        ("only clone -> build -> test from what git holds") and §4 R-5 / R-6. (sem: SEM-tools-069)
# where: runs inside WSL2 as testuser2. cargo exists there and nowhere else -- §5 of the
#        same doc: Windows native cargo is blocked by Smart App Control, and lake exists
#        only on the Windows side. This script therefore audits the cargo half only; the
#        lake half is printed at the end as an explicit gap, never silently skipped.
# gives: exit 0 only when the clone tested green AND enough of the suite actually ran.
#        Exit codes are never swallowed: the cargo status is taken from PIPESTATUS, and
#        nothing is sent to /dev/null.
# R-6  : this script IS the clone-E2E of R-6 as revised (05 §7b). "Clone-E2E as a doubt
#        probe" cannot exist -- it would re-enter cargo test from inside cargo test --
#        so the E2E lives here and doubt holds the in-repo consistency probes.
# usage: bash tools/e2e.sh
#        from Windows: wsl -d Ubuntu-24.04 bash -lc 'bash /mnt/c/.../glovrex/tools/e2e.sh'
#        GLOVREX_ALPHA=<path>   the out-of-tree subject crate tree (default: ../Glovrex_Alpha)
#        GLOVREX_E2E_KEEP=1     keep the temporary clone instead of deleting it
# exits: 0 green / 10 not the root of a git work tree / 11 temp dir on drvfs /
#        12 clone failed / 13 subject missing / 14 cargo missing / 15 target dir on drvfs /
#        16 cargo said ok but too little ran / 17 cargo said ok but the wrong suites ran /
#        18 the clone carries a committed symlink / 19 cannot enter the clone /
#        else cargo's own.

set -u

# --- thresholds ---------------------------------------------------------------------
# A harness that prints GREEN when nothing ran is not a gate: deleting every test file
# used to give `GREEN 0 probes over 0 suites (rc=0)` (req/08 §2 B-4, run E8). The floor
# is the measured suite -- 37 probes over 5 suites: term 9 + fold 8 + ledger 9 + writer 6
# + semantics 5 (36 as measured by the audit lane in req/08 §0, plus the spec_anchor probe
# that closed M-7). Raise these when probes are added; lowering one is a deliberate act,
# not a side effect.
#
# 2026-08-07, hand 5: the clone now builds and tests the whole workspace, not just probes/doubt.
# The two numbers move together with the suite list below -- req/29 §4: "adding a suite is two places
# at once: add the new suite name to EXPECTED_SUITES and raise MIN_PROBES at the same time. Doing only
# one hollows out the floor." Measured on 2026-08-07: 143 passing tests over 32 `test result:` lines (sem: SEM-tools-070)
# (37 doubt + 27 gx-core acceptance/property + 79 gx-canon, plus lib unit tests and doc-tests,
# which produce a `test result:` line each and are counted in the floor but have no
# `Running tests/<name>.rs` line and so are not named below).
# 2026-08-07, M2 hand 1: 166 passing tests over 38 `test result:` lines, measured by
# tools/verify_m2h1.sh. The move from 153/33 is +13 and +5: the thirteen tests of the new
# `m2_types` suite, and five result lines -- that suite plus a lib-unittest and a doc-test line
# each for gx-witness and gx-log, which hand 1 creates empty. Empty crates raise the floor by two
# lines apiece and by no tests, which is exactly what should happen: the floor counts what ran.
# 2026-08-08, M2 hand 2, step 1 (the gx-canon mint of E-M2-12): 179 passing tests over 39
# `test result:` lines. +13 and +1 -- the twelve of the new `mint_domain` suite plus the one
# `ac_014` gained when its single-road claim became a declared-road snapshot, and one result
# line for the new suite.
# 2026-08-08, M2 hand 2, step 2 (the self-written merkle log): 221 over 44, measured by
# tools/verify_m2h2.sh. +42 and +5: ac_021 (5) + ac_022 (9) + ac_023 (10) + ac_024 (8) +
# checkpoint_core (5) = 37 in five new suites, plus the five unit tests gx-log's `proof.rs` now
# holds -- those raise the count on the crate's existing lib-unittest line and add no line of
# their own, which is why the two numbers move by different amounts.
# 2026-08-08, M2 hand 3 (store.rs, fsync-on-append): 254 over 48, measured by
# tools/verify_m2h3.sh. +33 and +4. The four new suites are ac_069 (10), append_idempotence (5),
# audit_path_length (5) and map_key_order (3) = 23; the other ten are on lines that already
# existed -- checkpoint_core gains five for H2-4's `unsigned_checkpoint` and gx-log's
# lib-unittest line gains five for the path-length arithmetic `tile.rs` now holds.
# 2026-08-08, M2 hand 4 (gx-witness provenance.rs + evidence.rs): 285 over 52, measured by
# tools/verify_m2h4.sh. +31 and +4. Four new suites and nothing else: ac_015 (12),
# ac_016 (4), ac_017 (7), evidence_cid (8) = 31. gx-witness's lib-unittest and doc-test lines have
# existed since hand 1 created the crate empty and still carry no tests, so unlike hand 2 and hand 3
# the two numbers move by the whole of the new suites.
# 2026-08-08, M2 hand 5 (gx-witness dsse.rs + keys.rs + receipt.rs): 360 over 60, measured by
# tools/verify_m2h5.sh. +75 and +8. Eight new suites and nothing else: ac_018 (8), ac_019 (10),
# ac_020 (10), ac_070 (10), checkpoint_signature (9), pae_golden (8), receipt_kind_branch (9) in
# gx-witness, and base64_vectors (7) in gx-core = 71. The other four are on lines that already
# existed: gx-witness's ac_015/ac_016/ac_017 are untouched, so the remainder is gx-core's and
# gx-log's existing suites gaining cases where hand 5 widened them -- ac_008 (the twelfth module),
# ac_021 (`verify_inclusion_of` in the declared surface) and the doc-test lines of the two crates
# whose module documentation grew.
# 2026-08-08, M2 hand 6 (fuzz, audit/deny, coverage, mutation trial): 362 over 61, measured by
# tools/verify_m2h6.sh. +2 and +1. One new suite and nothing else: gx-canon's `unsafe_forbidden` (2),
# which fails the build if any shipped crate root stops declaring `#![forbid(unsafe_code)]`. The
# hand's other work adds no probe here on purpose -- the two fuzz targets live outside the workspace
# under `fuzz/` and are replayed by `ci.sh` stage 8, not by a `#[test]`, so the member count this
# script's sibling asserts stays at five.
# 2026-08-08, the M2 fix hand (req/38 §15/§16): 370 over 62, measured by tools/verify_m2h7fix.sh.
# +8 and +1. One new suite -- gx-log's `tile_wire` (3), which pins the canonical bytes of a `Tile`
# and is what decided E-M2-27(b) against renaming the `level` field. The other five are on lines that
# already existed: checkpoint_signature gains four for E-M2-26 (the bare-core signature being refused,
# the PAE shape, the payload-type separation, and the C2SP non-claim) and negative_vectors gains one
# for H6-4's declared-error assertion. H6-4's six new vector files add no probe of their own -- they
# are data read by suites that already run, which is why 18 vectors move the count by one.
# E-M2-29's third fuzz target and H6-9's dictionary add none at all: they live outside the workspace
# under `fuzz/` and are replayed by `ci.sh` stage 8, not by a `#[test]`.
# 2026-08-08, M3 hand 1 (gx-gate, A-1/A-10, E-M3-1/2/3/9): 393 over 67, measured by
# tools/verify_m3h1.sh. +23 and +5. Three new suites that hold probes -- gx-gate's `gate_shape` (9)
# and `verdict_order` (5), and gx-core's `m3_types` (6) = 20 -- and two that hold none: gx-gate's
# lib-unittest and doc-test lines, which count toward MIN_SUITES the way every other crate's do.
# The remaining three probes are on lines that already existed: `ac_069` gains two for A-1 (the
# impossible length header, and the ordering scan that is the half a mutation can fail) and
# `checkpoint_core` one for A-10 (the map key arithmetic 5 = 3 + 2).
#
# The cedar-policy dependency adds no probe of its own, on purpose: hand 1 measured it (req/61 §2)
# and calls none of it, and a test of an unused dependency is a test of cargo.
# 2026-08-08, M3 hand 2 (policy.rs, ASM-60-1, AC-025): 416 over 70, measured by tools/verify_m3h2.sh.
# +23 and +3, and the two numbers move together because all twenty-three probes are in the three new
# suites -- gx-gate's `ac_025` (4), `policy_mapping` (9) and `policy_determinism` (10). No existing
# line moved: `gate_shape` still holds nine, because hand 1's `the_gate_holds_nothing_yet` was
# rewritten rather than joined (the gate now holds a policy set, which is the thing that assertion
# existed to notice).
#
# Two of the twenty-three are proptest properties and are re-run by `tools/ci.sh` stage 4f at the
# case count 51 §3 names; they count once here, as every property does.
# 2026-08-08, M3 hand 3 (invariant.rs, AC-026/029): 433 over 73, measured by tools/verify_m3h3.sh.
# +17 and +3, and again the two move together because all seventeen probes are in the three new
# suites -- gx-gate's `ac_026` (5), `ac_029` (4) and `invariant_registry` (8). No existing line
# moved: `gate_shape` still holds nine, because the assertion the registry made false was rewritten
# rather than joined (`the_gate_holds_the_policy_half_and_nothing_else` became
# `the_gate_holds_the_two_halves_41_4_names_and_nothing_else`, req/38 §21 C-9 having ruled on the
# same move one hand earlier), and `ac_025` only lost a stale sentence from an assertion message.
#
# One of the seventeen is a proptest property, re-run by `tools/ci.sh` stage 4g at the case count
# 51 §3 names. The three source-scanning assertions inside `ac_029` count as the one probe they
# live in, as gx-log's `ac_069` ordering scan does.
# 2026-08-08, M3 hand 4 (verdict.rs: the meet, the escalation, the error vocabulary; A-8/A-9/F-3):
# 470 over 77, measured by tools/verify_m3h4.sh. +37 and +4. Four new suites -- gx-gate's
# `verdict_meet` (12), `error_vocabulary` (12), `proof_digest` (9) and `gate_input_spec` (4) = 37 --
# and three existing lines that moved in both directions, netting zero: `ac_025` gains one (a policy
# may now be named `cedar:default-deny`, which E-M3-11 stopped reserving), gx-canon's
# `negative_vectors` gains one (F-3's payload assertion), and gx-witness's `ac_016` **loses two** to
# B-4 -- its two spec-reading tests moved into `gate_input_spec`, where they compare 41 §4 with the
# real `GateInput` instead of with a stand-in. A suite whose count goes down is worth saying out
# loud: nothing was deleted, and the two assertions are named in the new file.
#
# Three of the thirty-seven are proptest properties, re-run by `ci.sh` stage 4h at the case count
# 51 §3 names. The two new vectors (D-65K, TR-1) add no probe of their own -- they are data read by
# suites that already run, which is why two vectors move the count by the one assertion that reads
# their payloads.
# 2026-08-08, M3 hand 5 (packs.rs: the shipped fs pack, its one road into a build, AC-028):
# 480 over 79, measured by tools/verify_m3h5.sh. +10 and +2, and the two move together because all
# ten probes are in the two new suites -- gx-gate's `ac_028` (4) and `pack_embedding` (6). No
# existing line moved: the pack file gained a disjunct (`/etc` itself) and a header, and neither
# changes what any earlier suite asserts -- `ac_025`'s two cases are `/etc/passwd` and `/tmp/x`,
# both decided the same way before and after.
#
# None of the ten is a property, so no `ci.sh` stage 4 line is added for them. Six of them are
# source scans (the road count, the `.cedar` inventory, the `@id` count, the effective-range
# clauses, the absence of a shipped invariant) and they count as the probes they live in, the way
# `ac_029`'s three scanning assertions do.
# 2026-08-08, M3 hand 6 (criterion + AC-064, the false-admit family, and E-M3-13 in gx-core):
# 500 over 81, measured by tools/verify_m3h6.sh. +20 and +2, and the two move together because all
# twenty probes are in the two new suites -- gx-core's `compose_range` (15) and gx-gate's
# `false_admit` (5). No existing line moved: E-M3-13 refuses metadata no fixture in this repository
# was using, so every suite that builds a `Transformation` builds the same one it did before.
#
# One existing generator did move, and it is worth saying where: `gx-canon/tests/support/mod.rs`
# draws `created_at` from `0..` instead of from the whole of `i64`, because predicate ① makes a
# negative timestamp a value the constructor refuses. That changes no count here -- the properties
# reading it assert the same things over the same number of cases.
#
# The criterion bench adds no probe, and the reason was measured rather than assumed: `cargo test`
# without target flags does not select bench targets at all, so `Running benches/verify_latency.rs`
# appears zero times in this script's log and the floor cannot move because of it. What does compile
# it is `tools/ci.sh` stage 2 (`clippy --all-targets`), which is where a bench that stopped building
# would be caught. Run explicitly (`cargo test --benches`) criterion answers in its one-iteration
# test mode and still prints no `test result:` line.
# 2026-08-09, the M3 fix hand (I-1/I-2, req/38 §26): 511 over 83, measured by tools/verify_m3fix.sh.
# +11 and +2, and the two move together because all eleven probes are in the two new suites --
# gx-gate's `verdict_identity` (8) and `deny_order` (3). No existing line moved: the fix adds tests
# and changes no shipped code, so every suite that ran before runs the same probes it did.
#
# The eleven exist because the hand-7 audit measured that three mutations of this crate's identity
# face were caught by nothing (req/67 §2.1 battery B-3, and the `verdict.rs:182` / `verdict.rs:510`
# survivors of `cargo mutants`). None of them is a new claim about behaviour: they are the A-10 form
# -- "count a projection's map keys and assert the count matches the declared number" -- asked of the three (sem: SEM-tools-071)
# projections gx-gate takes a digest through.
#
# The `RFC8949-MUST` value I-5 adds to `negative_vectors.rs`'s vocabulary moves no count: TR-1 was
# already read by the suites that read it, and a vector's `normative_basis` is data those suites
# check rather than a probe of its own.
# 2026-08-09, M4 hand 1 (the seventh member, and the three required-up-front DoD of req/69 §6.2): 543 over 91, (sem: SEM-tools-072)
# measured by tools/verify_m4h1.sh. +32 and +8. Six new suites hold thirty-one of the probes --
# gx-core's `value_range_closure` (4), `core_error_vocabulary` (3) and `m4_types` (10), gx-canon's
# `intent_identity` (6), and gx-substrate's `substrate_contract` (3) and `delta_skeleton` (5) -- and
# two of the new lines hold none: gx-substrate's lib-unittest and doc-test lines, which count toward
# MIN_SUITES the way every other crate's do (the shape gx-gate's arrival had in M3 hand 1).
#
# The thirty-second probe is on a line that already existed: gx-core's doc-test line goes from two
# to three, for the `compile_fail` block that shows `Fingerprint` has no `PartialEq` (E-M4-15). That
# is the M1 idiom for a negative compile claim (51 §2 admits it in place of trybuild) and the reason
# M4-20 ruled trybuild out -- a doctest adds no package.
#
# `compose_range` still holds fifteen. Its pin `identity_is_a_third_door_and_this_hand_did_not_close_it`
# was **rewritten, not joined**: E-M3-18 inverted the answer it pinned, so the probe asserts the
# refusal now under the name `identity_is_the_third_door_and_e_m3_18_closed_it`. A suite whose count
# stands still while its meaning turns over is worth saying out loud (req/38 §21 C-9 ruled on the
# same move in M3 hand 2).
#
# 2026-08-09, M4 hand 2 (`SubstrateAdapter`'s 7 methods, contract table, AC-046, E-M4-27): 559 over 94, (sem: SEM-tools-073)
# measured by tools/verify_m4h2.sh. +16 and +3. Three new gx-substrate suites hold thirteen --
# `adapter_spec` (5: parses 41 §4's markdown and cross-checks it against the real type -- the I-11
# shape), `adapter_contract` (4: verbatim check of the contract table's 7 rows) and `ac_046` (4: paired check of 2 positive examples + a negative-example doctest) -- and two of the remaining three (sem: SEM-tools-074)
# probes are on a line that already existed: gx-substrate's doc-test line goes from zero to two, for
# AC-046's running example and its `compile_fail` twin (M4-20 (b): the package does not build,
# because trybuild is not adopted). (sem: SEM-tools-075)
#
# The sixteenth is `m4_types`, which goes from ten to eleven. **E-M4-27** made `cas_eq` refuse a
# comparison across substrates, so hand 1's pin was rewritten in place -- the same operation §29
# M4H1-9 confirmed for the door pin -- and one probe was added for which of the two refusals answers when (sem: SEM-tools-076)
# both fields disagree. A rewritten pin is a probe that did not move a count while changing its
# meaning, which is the thing this comment exists to record.
# 2026-08-09, M4 hand 3 (delta semantics + `gx-substrate-conformance` + the facts window's settlement): 609 over 105, (sem: SEM-tools-077)
# measured by tools/verify_m4h3.sh. +50 and +11. Nine new suites hold forty-seven of the probes --
# gx-canon's `fingerprint_identity` (6), gx-substrate's `delta_semantics` (6), `substrate_error` (9)
# and `planned_delta_identity` (7), and the eighth member's `harness_shape` (7), `contracts_seven`
# (3), `laws` (4), `opacity` (3) and `residual` (2) -- and two of the eleven new lines hold none:
# gx-substrate-conformance's lib-unittest and doc-test lines, which count toward MIN_SUITES the way
# every other crate's do (the shape gx-substrate's arrival had in M4 hand 1).
#
# The other three probes are on lines that already existed. gx-substrate's lib-unittest line goes
# from zero to two, for the two `#[cfg(test)]` probes beside **E-M4-28**'s new `Error` (the `From`
# conversion and the seven kinds), and `adapter_spec` goes from five to six for
# `e_m4_28_the_bare_result_is_this_crates_own`. That sixth probe is worth naming here: the seven trait
# signatures are byte-identical before and after the `Result` swap, so the suite that compares 41 §4's
# text with the source stayed green through a change of meaning, and the new probe measures the
# import instead.
#
# `unsafe_forbidden` still holds two: the eighth crate root was added to its list rather than the
# walker being taught an exception (E-M4-19 makes the harness `publish = false`, and a root is a root).
# 2026-08-09, M4 hand 4 (`gx-adapter-fs`: the first real adapter, AC-047, the scope bound and the
# is_complete split): 660 over 115, measured by tools/verify_m4h4.sh. +51 and +10. Six new suites in
# the ninth member hold forty-two of the probes -- `locator_normalisation` (13: E-M4-12's clauses,
# L7's two property halves and the 42 §2.3 doc scan), `fs_delta` (8: canonical DAG-CBOR, the free
# monoid, and the two refusals of M4-13(a)), `scope` (6: ASM-69-1), `conformance` (5: 51 §7's harness
# pointed at a real adapter), `ac_047` (4) and `plan_purity` (4: the two "absence" scans) -- plus (sem: SEM-tools-078)
# gx-adapter-fs's lib-unittest line (2, the locator edge cases) and its doc-test line (0 probes, and
# a line all the same, the shape every member's arrival has had).
#
# The other nine are in two crates this hand had to touch: gx-core's `scope_bound` (4, M4H1-2's bound
# at both sides of the boundary) and gx-substrate's `scope_elision` (5, the digest half of the same
# ruling, in the `fingerprint.rs` module 41 §2 always had and E-M4-1 re-read as "where the computation lives"). (sem: SEM-tools-079)
#
# 🔴 The measurement itself has a gotcha worth the line: `cargo test --workspace` output redirected
# to a file **on /mnt/c** loses lines under parallel test binaries -- the same run measured 544/95
# through a 9p redirect and 660/115 written to a WSL-native path. Every count in req/73 is from the
# native log. A floor read through the wrong filesystem reads low, which is the direction that hides
# a suite that did not run.
# 2026-08-09, M4 hand 5 (`apply` / `invert`, AC-048, AC-049, the escrow ceiling and the rewritten
# disclosure): 681 over 119, measured by tools/verify_m4h5.sh. +21 and +4, and every one of the four
# new lines is a new suite in the ninth member -- `apply_durability` (9: the three steps in order, the
# temporary file that does not survive, L2 by behaviour, the two refusals, and the two disclosure
# scans M4H4-10 asks for), `ac_049` (6: create/change/delete in T-10b's order, L5's two roads, the (sem: SEM-tools-080)
# postcondition's scope, and a property over generated Givens), `ac_048` (3: the ceiling's `Ok(None)`,
# the gate's `Escalate`, and the control that makes it mean something) and `invert_ceiling` (3:
# either side of the bound, M4H2-8's 1:1 probe, and the second `Ok(None)` reason).
#
# No line that already existed moved. `conformance` still holds five probes and says something
# different with them: 13 of 15 obligations pass where hand 4 had 7, and `is_complete` is still false
# because `commutation` is hand 6. A suite whose count stands still while its meaning turns over is
# worth the sentence (§21 C-9, and the third time this file has had to write it).
# 2026-08-09, M4 hand 6 (`commutation`, AC-052, AC-053, the forward ceiling, and the two refusals
# E-M4-32 / M4H5-5 named): 705 over 122, measured by tools/verify_m4h6.sh. +24 and +3. Three new
# suites in the ninth member hold twenty-two of the probes -- `ac_052` (12: 51 §7's commutative/non-commutative pair, (sem: SEM-tools-081)
# M4-14's residual discrimination and its negative, M4-25's symmetry and reflexive case, the two
# spellings of one position, the foreign delta in either slot, the scan that says this answer reads no
# filesystem, and a property over generated spellings), `ac_053` (6: AC-029's three faces applied to
# "not by way of the engine/gate path", plus the control that keeps each one from being a fact about the (sem: SEM-tools-082)
# repository) and `forward_ceiling` (4: either side of M4H5-4(b)'s bound, the M4H2-8 1:1 probe, the
# relation to the escrow ceiling, and the hole the ruling left at `decode`).
#
# The other two are in suites that already existed: `fs_delta` gains the `NotAPosition` refusal
# (M4H5-5 adopted (b)) and gx-substrate's `substrate_error` the probe that says the vocabulary is ten words. (sem: SEM-tools-083)
# `conformance` and `invert_ceiling` hold their counts and change their meaning -- 15 of 15
# obligations pass where hand 5 had 13, `is_complete` is **true** for the first time, and the pin that
# said "a `pre` naming another position is `Ok(None)`" now says `Err(LocatorMismatch)` (E-M4-32, the (sem: SEM-tools-084)
# C-9 rewrite). Two suites whose numbers stand still while their meaning turns over: the fourth and
# fifth time this file has had to write that sentence.
# 2026-08-09, M4 fix batch (req/38 §35's seven): 720 over 125, measured by tools/verify_m4fix.sh. (sem: SEM-tools-085)
# +15 and +3. Three new suites: `fault_injection` (4: E-M4-35's three places, each with the fault
# that actually reaches it, plus the source scan that holds the rule rather than the three
# instances), `broken_fixture` (6: the negative control the 51 §7 harness never had -- eighteen
# deliberate flaws, one per obligation, which is what took gx-substrate-conformance's line coverage
# back over 51 §14's 80%) and `floor_doubt` (3: the gate's own numbers, below). The other two probes
# are in suites that already existed -- `forward_ceiling` and `invert_ceiling` each gain the case
# **on** the bound, which is the byte `>` and `>=` disagree about and the reason two mutants lived
# through hand 7.
#
# 🔴 This floor now lives in a **fourth** place: `probes/doubt/tests/floor_doubt.rs` reconstructs
# both numbers from the source (test functions + doc-test fences; test files + lib targets counted
# twice + `[[bin]]` targets) and asserts they equal the two constants below. req/76 §2.1's (C-1) put
# these back to hand 5's 681/119 and the clone gate still printed GREEN, because nothing in the
# repository compared the declaration with the world. Raising the floor is now three edits in this
# file and none anywhere else -- the fourth place checks, it does not have to be updated.
# 2026-08-09, M5 hand 1 (`gx-engine`: the tenth member, the journal and DR-2 two axes): 770 over
# 134, measured by tools/verify_m5h1.sh. +50 and +9. Six new suites hold thirty-nine of the probes
# -- gx-engine's `journal_vocabulary` (8: parses 42 §3.13 and 43 §3 and cross-checks them 1:1 against
# the real type -- where E-M5-1/E-M5-3 are measured), `journal_roundtrip` (9: DoD ④'s property + every
# cut of a torn tail + both sides of the ceiling + the write-ahead order, from source), `journal_identity`
# (6: I-1/A-10 across 12 variants, 38 fields' worth), `engine_shape` (6: 41 §2's module table + error
# vocabulary + three "absence"s), `escrow_types` (6: parses 42 §3.12 and cross-checks it) and gx-core's `enforcement_axes` (6: DR-2's two axes) -- plus `workspace_doubt` (3) in probes/doubt, (sem: SEM-tools-086)
# which is the membership gate no crate can assert about itself. Six more are unit tests inside
# src (`store.rs` 2, `replay.rs` 2, gx-core's `enforcement.rs` 2), and two of the nine new suite
# lines hold no probes at all: gx-engine's lib-unittest and doc-test lines, which count toward
# MIN_SUITES the way every other crate's do (the shape gx-substrate's arrival had in M4 hand 1).
#
# No doc-test moved: this hand added no fenced example, so the five doc-tests are still M4's five.
# M5 hand 2 (T-1..T-4e + T-8/T-8r, AC-030..033): 770/134 -> 803/140. **+33 and +6.** Five new suites
# hold thirty of the probes -- gx-engine's `ac_030` (5: two-stage identity, twice in one process and
# once in another via the `engine_id_probe` [[bin]]), `ac_031` (4: the delta and Fingerprint₀, and
# the retrieval 43 T-10a will make), `ac_032` (8: the three verdicts, plus T-4d/T-4e and the gate's
# ⊥ under both enforcement modes), `ac_033` (7: T3's guard, the injected broken canon, T-8r) and
# `lifecycle_states` (3: 43 §1 and §3 parsed out of the spec and compared) -- and the sixth suite
# line is the probe binary's own `unittests` line, which holds no probes and counts toward
# MIN_SUITES the way gx-canon's `cid_probe` does. The remaining three probes are additions to
# suites that already existed: `engine_shape` (+1, the single-producer scan for
# `VerifierUnavailable`), `error_vocabulary` (+1, E-7's retirement) and `journal_vocabulary`
# (+1, the Verdict record's divergence from 42 §3.13).
#
# No doc-test moved: this hand added no fenced example either.
#
# M5 hand 3 (store / escrow / replay, AC-039): 803/140 -> 834/144. **+31 and +4.** Four new suites
# hold all thirty-one -- gx-engine's `ac_039` (8: bit-equality for a three-row script, the T-4e and T-8r
# rows that are easiest to lose, both control experiments, the counting adapter, and the property),
# `blob_store` (9: the CID key, "reference-only" measured by damage that survives a second put, the digest (sem: SEM-tools-087)
# check, M4H6-3's shared CID, both ends of the ceiling on the bound, and the escrow round-trip),
# `sigma_replay` (7: the components only hands 4 and 6 can write, from journals written by hand) and
# `store_shape` (7: one store, one ceiling per receiving mouth, the contract row, Σ in the read-only
# module, no adapter in it, no second `Recovery`, and `sigma` not reading its own journal). Nothing
# was added to a suite that already existed.
#
# 🔴 The thirty-first arrived from the mutation battery rather than from the plan: `ac_039`'s
# "a degraded admission is unenforced **before** it is canonicalised". Mutation (g) broke the (sem: SEM-tools-088)
# `Verdict` arm's `enforced = false` and nothing failed, because every T-4e fixture walked on to T-8
# and the later record repaired the earlier one's damage. A masked claim is not a covered one.
#
# No doc-test moved: this hand added no fenced example either.
#
# M5 hand 4 (the commit critical section, AC-034/035/038): 834/144 -> 870/149. **+36 and +5.** Five
# new suites hold thirty-six of the probes -- gx-engine's `commit_protocol` (18: the Rule 2 scan, the (sem: SEM-tools-089)
# journal-first ordering, E-M5-1's pairing, M5-24's two reasons, M5-25's single producer, I-3's
# count, E-M4-31's `Timestamp(0)`, M5H3-4's frontier-and-root, discipline 48's three intermediate stops, (sem: SEM-tools-090)
# Σ against a journal an execution wrote, the provenance field by field, M5H3-1(c)'s clock, the
# receipt verified against the ledger, M5H4-3's refusal, M5H3-7's mirror count, the reopened engine,
# and the older fixture), `ac_034` (4: the injected concurrent writer, the journal, the control, and
# `Fingerprint₀` staying put), `ac_035` (3: the counter through every non-Committing state, the
# `Verifying` state via the journal, and the wrong-state refusal with T-9's idempotency),
# `ac_038` (6: both rollback outcomes, both announcements, the escrowed body, the receipt that is
# not issued, and `NotAttempted`'s unreachability) and `ledger_durability` (3: K-9's EACCES through
# gx-log and through the engine, plus the control). The remaining two are additions to
# `journal_vocabulary`, which gained the two divergence probes M5-25's record and AC-038's rollback
# outcome need.
#
# No doc-test moved: this hand added no fenced example either.
#
# M5 hand 5 (43 §7's crash recovery, AC-043, K-6, the real-binary E2E): 870/149 -> 888/154.
# **+18 and +5.** Four new suites hold all eighteen -- gx-engine's `crash_recovery`
# (10: the procedure's one entry and one worker, Λ4's CAS unreachable from the recovery with the
# control that it is still in `commit`, E-M5-1's field being read, the write-ahead ordering of the
# re-application, `RECOVERY_PATHS` against its enum, the test-only binary, discipline 48's stop at the (sem: SEM-tools-091)
# crashed state, Λ4's two recoveries of one directory, 43 §7-3b's re-issue with M5H4-7's
# `payload_matched`, and M5H3-5's measurement of what a restart restores), `ac_043` (4: three
# injection points at ten trials each, plus the recovery's own idempotence), `concurrent_commit`
# (2: K-6's two threads on one transformation and on one object) and `binary_e2e` (2: AC-034 across
# a real process boundary, and 51 §8.1's four processes). `unsafe_forbidden` adds no probe and gains
# a row: its walker names twelve crate roots now, because a second `[[bin]]` exists.
# 🔴 The five suite lines are **four suites plus one binary**: a `[[bin]]` raises the `test result:`
# count by one and the probe count by none, which is the same arithmetic M2 hand 1's empty crates
# had. The floor counts what ran.
#
# No doc-test moved: this hand added no fenced example either.
#
# M5 hand 7 (the three benches, 51 §14's branch coverage, M5H1-7): 944/167 -> 952/168. **+8 and +1.**
# One new suite holds five of them -- gx-engine's `state_machine_coverage` (2 lint probes over
# `tests/state_machine_coverage.md`, which 51 §14 names by path, and 3 that walk 43 **T-13** by
# injection: `cas_eq`'s two refusals through a mis-wired fixture adapter, and the from-set measured
# at two of its eight states). Two more are additions to `workspace_doubt` (M5H1-7 adopted (a): the (sem: SEM-tools-092)
# member count in three declarations and one measurement, and the member **names** in all four
# places -- a count that agrees against the wrong list is a green stage 0). The eighth is
# `floor_doubt`'s `f4`, which is the gate-gate for 51 §14's other half: the lint has to be a **named**
# ci stage, because (C-2) showed that a gate running only inside a nine-hundred-probe total is a gate
# whose disappearance nobody notices.
#
# 🔴 **The three benches add nothing to either number, and that is correct.** `cargo test` does not
# build a `[[bench]]` target, and a `harness = false` bench prints no `test result:` line even when
# it is built -- measured on this tree: gx-gate's `verify_latency` has existed since M3 hand 6 and the
# floor has never counted it. So `benches/{commit_pipeline,throughput,journal_recovery}.rs` and
# `benches/support/mod.rs` are four new files and zero new probes. What gates them is `tools/ci.sh`
# stage 10, which is opt-in, and the numbers they print live in req/85 §2.
#
# No doc-test moved: this hand added no fenced example either.
#
# M5 hand 8 (the audit hand): 952/168 -> 955/169. **+3 and +1.**
# All three are the one new suite, gx-engine's `ac_042` -- the criterion §44 found had been ruled
# and never implemented (M5-15 adopted (b), and 34's only M5 acceptance criterion still missing when hand 7 (sem: SEM-tools-093)
# finished): the model-based property over generated traces, a negative probe that feeds the
# criterion function five roads and requires it to reject three of them, and the seed-file inventory.
# 🔴 Nothing else this hand did adds a probe, and the reasons are worth naming because an audit hand
# that grew the floor by its own measurements would be counting its instruments as coverage:
# `cargo mutants` and `cargo llvm-cov` are external runs whose output lives in req/86 §3 (they build
# their own trees and print no `test result:` line here); `tools/audit_gate_probe.sh` is a shell
# probe run by `tools/verify_m5h8.sh` and by no cargo target; the `GLOVREX_BENCH_SHARDS` arm is in a
# `[[bench]]`, which `cargo test` does not build (hand 7's paragraph above); and the §7-3b record-
# sequence assertion is **an addition to an existing probe** in `crash_recovery`, not a new one.
#
# M5 fix hand: 955/169 -> 968/170. **+13 and +1.**
# The suite is `overclaim_doubt` (probes/doubt), which is M5H8-6: 45 §4.1's forbidden sentences,
# read out of 45 at run time and looked for in every `.md` file and every `.rs` comment of this
# repository. Two probes -- the scan, and the positive control that finds all seven in 45 itself, so
# that a scan which matched nothing cannot pass for a repository that says nothing wrong.
# The other eleven probes are additions to suites that already ran, which is why the suite count moves
# by one and not by five:
#   +3 `gx-witness/tests/receipt_kind_branch.rs` -- M5H6-8(1)'s pairing rule, its negative control,
#      and the producer side (`Receipt::issue` refuses to sign what its schema refuses);
#   +4 `gx-witness/src/dsse.rs`'s new `#[cfg(test)] mod tests` -- M5H8-9's three visitor roads and
#      the `expecting` message. These land in the **gx-witness lib unittest binary**, which has
#      printed a `test result:` line with 0 passed since M2, so the suite count does not move;
#   +2 `gx-log/src/store.rs`'s new `#[cfg(test)] mod tests` -- M5H8-10's two counters, in gx-log's
#      lib unittest binary, which already carried `proof.rs` and `tile.rs`'s probes;
#   +1 `gx-log/tests/ac_069.rs` -- the `#[cfg(not(unix))]` arm's declared gap, as a source scan;
#   +1 `gx-engine/tests/ac_033.rs` -- M5H8-11's return-value check with its two goldens.
# 🔴 Two things this hand changed add **no** probe, and saying so is the point: M5H8-12 re-aimed an
# existing probe in `crash_recovery` (the assertions its name already promised) rather than adding
# one, and `rust-toolchain.toml` is a pin, not a test. A fix hand that counted a re-aimed probe as a
# new one would be reporting the same probe twice.
# 2026-08-10, M6 hand 1: 1005 over 181. +37 and +11.
#   +4  `gx-canon/tests/authority_boundary.rs` — Rule 1's three counters (req/88 §3 Λ1) plus the (sem: SEM-tools-094)
#       positive control that keeps them from reporting 0 about nothing;
#   +5  `probes/doubt/tests/m6_surface_doubt.rs` — the `.gx/` table against req/56, the `supersede`
#       reservation of §47, M6-21's arming, M6-22's dependency, discipline 46's library; (sem: SEM-tools-095)
#   +8  `gx-cli/tests/draft_index.rs` and +7 `gx-cli/tests/gx_layout.rs` — M6-01(a), M6-02(b), req/56;
#   +6  `gx-engine/tests/id_resolution.rs` — M6-02(a), including E-M5-13 on the live path;
#   +2  `gx-api/tests/router.rs` — the fourteen named endpoints and the zero served;
#   +3  on lines that already existed: `workspace_doubt` (M5FIX-1(c)'s root scan), `journal_vocabulary`
#       (E-M5-13's fields) and `engine_shape` (the inverse is an accessor);
#   +2  gx-cli's lib unit tests (the consumer declarations of M6-21/M6-22 assert their own shape).
# The eleven lines are those six suites plus gx-cli's and gx-api's lib-unittest and doc-test lines,
# which count toward MIN_SUITES the way every other crate's do, and gx-cli's `gx` binary, whose
# unittest line is the fourth `[[bin]]` in this workspace.
#
# 🔴 **This hand raised the floor although its brief said hand 7 owns it, and the reason is C-1.**
# req/88 §6.2 puts "raise `MIN_PROBES`/`MIN_SUITES` **at the same time**" in hand 7, but (sem: SEM-tools-096)
# `probes/doubt/tests/floor_doubt.rs::f1` asserts **equality** between this declaration and an
# independent reconstruction from the source — deliberately, because "a floor below the real count is
# a gate a repository can pass with its tests removed". Under an equality gate the floor is a (sem: SEM-tools-097)
# *derived* number: any hand that adds a probe and leaves it behind is red immediately, and there is
# no state in which a hand can add probes and defer the raise. Hand 7's line has to be read as the
# **audit** of the declaration ("cross-checking the declaration site against the measurement") rather than as sole custody of the number. (sem: SEM-tools-098)
# Raised as **M6H1-4**.
# 2026-08-10, M6 hand 2: 1045 over 190. +40 and +9 over hand 1's 1005/181.
#   +3  `probes/doubt/tests/m6_surface_doubt.rs` §D (the RED commit) — the read-side verbs are
#       declared, the exit table equals 44 §1.2's per-section lists, and `Receipt::signature_for`
#       has a call site (M6-22 adopted (b), the accessor hand 1 armed and this hand pays for); (sem: SEM-tools-099)
#   +5  `gx-cli/tests/ac_057.rs` — AC-057's two cases through the binary, the unanchored case that
#       H5-9 refuses to call a pass, and the structural half of "not wired";
#   +2  `ac_018_cli.rs` / +2 `ac_019_cli.rs` / +2 `ac_020_cli.rs` — 51 §15 M6 row's three (sem: SEM-tools-100)
#       re-confirmations, now through `gx receipt verify` and `gx key gen` rather than the library;
#   +6  `receipt_disclosure.rs` — M6-16's four levels, M6-22's consumer, the four-value
#       `checks.inclusion`, and the `.gx/receipts/` store (M6H2-1);
#   +6  `key_surface.rs` — M6-29's leak scan with its positive control, and M6H2-10;
#       +5  `log_commands.rs` — `proof`/`consistency`, and M6-24 adopted (b)'s checkpoint producer;
#       +5  `replay_cmd.rs` — M6-26 adopted (a)'s `{matches, diffs}`, in both directions;
#       +5  `exit_map.rs` — discipline 52 on the binary, and every `gx_code` against 44 §2.3's twelve; (sem: SEM-tools-101)
#   -1  net of the above: `gx-cli`'s lib unit tests lost none and gained none, and the
#       `#[cfg(unix)]` probe in `key_surface.rs` is counted from the source the way every other
#       `#[test]` is (`floor_doubt` reads attributes, not runs).
# The nine lines are the nine new files under `crates/gx-cli/tests/`; `tests/support/mod.rs` is a
# module of the binaries beside it and not a target, which is why it adds none.
# 2026-08-10, M6 hand 3: 1073 over 196. +28 and +6 over hand 2's 1045/190.
#   +3  `crates/gx-cli/tests/ac_054.rs` — AC-054's four verbs, normal and abnormal, with the status
#       written as a number, plus the two places 44 §1.4's 2 is now returned;
#   +3  `ac_030_cli.rs` — AC-030/AC-011's CLI re-confirmation across two projects that share
#       nothing, and 44 §0's two spellings of an id resolving to one transformation;
#   +7  `pipeline_cmds.rs` — the draft round-trip and its negative half, Λ2's ten journal records,
#       the receipt store `gx commit` fills, the flags with nowhere to go, the substrate moving
#       under a resume, `--idempotency-key`, and `--evidence` as JSONL;
#   +4  `exit_matrix_cli.rs` — M6-25 from the implementation's side;
#   +2  `crates/gx-engine/tests/record_only_per_call.rs` — M6-08 adopted (a)'s argument overriding one (sem: SEM-tools-102)
#       evaluation and setting nothing, and T-8r with its control;
#   +5  `probes/doubt/tests/m6_exit_matrix.rs` — 44 §1.2's thirteen exit lists against §1.4's eight;
#   +4  `m6_surface_doubt.rs` §E — the pipeline verbs, Rule 2's two counters, the absence of any way (sem: SEM-tools-103)
#       to tell the binary what time it is, and the receipt store's writer.
# The six lines are those six new suite files. `tests/support/mod.rs` gained the pipeline fixture and
# is still a module rather than a target, so it adds none.
# 2026-08-10, M6 hand 4: 1103 over 202. +30 and +6 over hand 3's 1073/196.
#   +6  `crates/gx-cli/tests/undo_cmd.rs` — AC-054's last subcommand: the substrate coming back, an
#       unknown id at 6, the flag with nowhere to go, **M6-25's 2 on a denied undo**, 43 T-12's
#       guard read out of the journal alone (E-M5-13's `parents`), and the re-identification that
#       refuses a draft which is not the one the transformation was planned from;
#   +6  `ac_071_072_cli.rs` — AC-071 by ticket id and AC-072 by transformation id (M6-04 adopted (c)'s two (sem: SEM-tools-104)
#       spellings), an unknown ticket at 6, a blank reason, and the ruler M6H4-6 requires;
#   +6  `ac_073_cli.rs` — 43 T-7 from `Candidate` and from `Escalated`, the committed refusal
#       answered by T-7 rather than by a resume, **E-M6-1's draft refusal**, 6, and `--actor-key`;
#   +6  `policy_cmds.rs` — M6-21's three accessors doing work an operator can read, the empty-set
#       warning, two load refusals, and `gx policy test` with a **failing** expectation;
#   +2  `record_only_e2e.rs` — DR-2's two postures on one transformation (M6H3-9, the E2E hand 3
#       could not write without a pack that denies a writable path);
#   +2  `crates/gx-engine/tests/ticket_rehydration.rs` — M6H3-10 adopted (b)'s measurement, kept: the (sem: SEM-tools-105)
#       ticket a rehydrated row recovers, and the reverse map of 43 T-4c's 1:1 declaration;
#   +2  `exit_matrix_cli.rs` — the ruled additions table (M6-25) and its citation check;
#   +0  `m6_surface_doubt.rs` §F — two new probes, and two of hand 3's tightened rather than added
#       (the M6-21 count is now an assertion), so the file's own line moves by the two.
# The six lines are the six new suite files (`undo_cmd`, `ac_071_072_cli`, `ac_073_cli`,
# `policy_cmds`, `record_only_e2e`, `ticket_rehydration`). `tests/fixtures/deny-writable.cedar` is
# data read at run time and adds none.
# 🔴 **M6 hand 5** (44 §2's thirteen synchronous endpoints, the gx_code map, auth, Idempotency-Key,
# AC-055). +44 probes over +7 suites:
#   +7  `m6h5_cli.rs`           — E-M6-13's two exits + the reservation's negative half, M6H4-7's two
#                                 receipt kinds, M6H3-2's proof and reasons (the red-first part);
#   +3  `ac_055.rs`             — AC-055 itself, Λ2's draft asymmetry as a count, and the two
#                                 receipt vocabularies compared;
#   +7  `auth.rs`               — M6-10: the walk over twelve routes, the wrong token, the token
#                                 comparison, the unset token, the bind policy, the absence notice,
#                                 and E-M6-7's start-up refusal on a keyid the project does not record;
#   +11 `endpoints.rs`          — the pipeline over HTTP plus the refusal each endpoint names;
#   +6  `idempotency.rs`        — M6-11's four claims plus the one it is **not** about;
#   +3  `rule_two.rs`           — Rule 2's two counters and Rule 1's manifest half, for gx-api; (sem: SEM-tools-106)
#   +6  `m6_gx_code.rs`         — M6-09: 44 §2.3's twelve parsed and compared, the denominator (33
#                                 refusal kinds), the folds, and INVALID_STATE's absence (M6H5-3);
#   +1  `router.rs`             — hand 1's two became three (the routed walk).
# The seven lines are the seven new suite files. `receipt_disclosure`, `pipeline_cmds`, `ac_073_cli`
# and `exit_matrix_cli` changed assertions rather than counts.
# 🔴 **M6 hand 6** (`GET /stream`, `gx serve`'s runtime and graceful shutdown, M6-05's four
# extensions, E-M6-14's `gx draft discard`, E-M6-20's commit body). +35 probes over +7 suites:
#   +7  `crates/gx-api/tests/stream.rs`     — AC-056 through the router, the resume cursor's three
#                                             entrances, and M6-12's map against the **engine**;
#   +7  `crates/gx-api/tests/lists.rs`      — M6-05's four endpoints, 44 §2.7's limit, and the probe
#                                             that shows the list cursor **is** the stream cursor;
#   +5  `crates/gx-api/tests/shutdown.rs`   — the three stages, the exit that may not be 0, and the
#                                             subscription that has to end for a shutdown to finish;
#   +3  `crates/gx-api/tests/dr2.rs`        — E-M6-20: a denied change applied over HTTP with
#                                             `enforced:false`, and "absent" vs "false"; (sem: SEM-tools-107)
#   +5  `crates/gx-cli/tests/m6h6_cli.rs`   — E-M6-14, M6H5-13's `--help`, the bind and TLS refusals;
#   +2  `crates/gx-cli/tests/ac_056.rs`     — AC-056 over a **socket**, and SIGTERM's three stages;
#   +4  `probes/doubt/tests/m6_stream_map.rs` — the vocabulary against 44 §2.2 and 33 NFR-018, and
#                                             the stale L343 that E-M5-8 keeps stale on purpose;
#   +2  `crates/gx-engine/tests/record_only_per_call.rs` — E-M6-20's engine half, both directions.
# The seven lines are the seven new suite files; the engine pair joins a suite that existed.
# 🔴 The declaration moved by **35** and the reconstruction by 35 only after `floor_doubt` was taught
# `#[tokio::test(flavor = ..)]` — seventeen of these were invisible to it, which is M6H5-10's defect
# in a second spelling (M6H6-11).
# 🔴 **M6 hand 7** (benches, CI wiring, the distributables, and "cross-checking the declaration site against the measurement"). +N probes (sem: SEM-tools-108)
# over +M suites, and the hand's own line about the number is that it is an **audit** rather than a
# raise: §48 M6H1-4 adopted (a) reads req/88 §6.2's "raising the floor" as "cross-checking the declaration site against the measurement (an audit)", because (sem: SEM-tools-109)
# under `floor_doubt`'s equality gate the floor is derived and no hand can defer it.
#   +5  `crates/gx-engine/tests/subject_index.rs` — M6-07 adopted (b)'s index against a full scan (the (sem: SEM-tools-110)
#       oracle), the empty answer for an unknown subject, 43 §8's wait, the restart, and the third
#       door into the table (`rehydrate_committed`, M6H4-4);
#   +4  `crates/gx-api/tests/m6h7_api.rs`        — E-M6-22's `UNAVAILABLE` and the transcription that
#       stays twelve rows, M6H5-12's version accessor, M6H6-15's `inverse_status`;
#   +6  `probes/doubt/tests/m6h7_delivery.rs`    — 47 §1(a)/(c): the compose file, the musl target and
#       its `ldd`-equivalent evidence, the wasm PoC's recorded answer, and **the four declaration
#       sites reconciled against the measurement** (M5H1-7's shape, this hand's own DoD).
# `crates/gx-api/benches/serve_throughput.rs` and `crates/gx-engine/benches/recover_cost.rs` add
# **none**: a bench is not a `tests/` target and `floor_doubt` counts test attributes.
# 🔴 **M6 fix batch** (req/38 §55: M6H8-1~19, the confirmed six, and discipline 53's three flags). **+14 (sem: SEM-tools-111)
# probes over +2 suites**, and every one of them is an instrument rather than a feature — this batch
# raised no new capability except `--checkpoint-key`:
#   +2  `crates/gx-witness/tests/witness_error_vocabulary.rs` — **M6H8-16 adopted (a)**, and (sem: SEM-tools-112)
#   +2  `crates/gx-log/tests/log_error_vocabulary.rs`         — the pair §52's M6H5-2 (b) left
#       unpaid: `ERROR_KINDS` against the enum, in the two crates where hand 8's rename mutation
#       survived (the other four already had it);
#   +4  `crates/gx-canon/tests/authority_boundary.rs` — **M6H8-1 adopted (a)**: A2 (a `//` inside a string (sem: SEM-tools-113)
#       is not a comment — 44 §2.3 makes URIs mandatory, so the shipped code produces the input that
#       blinded the scanner), A1 (cargo's `package =` rename), A6 (the surface list derived from the
#       workspace and compared with the written one), and the **unclosed** attacks A3/A4 written down
#       as the text gate's limits rather than papered over;
#   +1  `probes/doubt/tests/floor_doubt.rs` — the attribute counter reads syntax instead of a list of
#       spellings, with M6 hands 5 and 6's two accidents as its negative controls;
#   +2  `crates/gx-api/tests/dr2.rs`  — **M6H8-14 ④**: `enforced` on the undo answer, and the undo of
#       a **denied** inverse, which nothing had ever driven — it deadlocked the server;
#   +3  `crates/gx-cli/tests/ac_057.rs` — **M6H8-11**: `anchor_authenticated` on every answer, and
#       `--checkpoint-key` refusing a forged head.
# The two lines are the two new suite files; the rest join suites that existed.
# 🔴 **M7 hand 0** (req/98 §7-2, ruling #4 / #25 / additional ruling b). **+1 probe over +0 suites**: (sem: SEM-tools-114)
#   +1  `probes/doubt/tests/floor_doubt.rs` — **f6**: the floor `README.md` shows a reader against
#       the floor this file declares. ruling #25's origin is that the README said 968/170 while the (sem: SEM-tools-115)
#       gate said 1211/221, and prose was the one spelling nothing compared with anything.
#   +0  `crates/gx-api/tests/dr2.rs` — **FR-M7-1** reverses an expectation rather than adding one:
#       the record-only undo probe asserted `403` and now asserts `200`+`enforced:false`. A
#       reversal moves no number and is therefore invisible here, which is why hand 0 states it in (sem: SEM-tools-116)
#       this comment instead of leaving the count to say it.
# 🔴 **M7 hand 1** (req/98 §7-2, FR-045 / AC-050). **+34 probes over +7 suites** — the thirteenth (sem: SEM-tools-117)
# member, `gx-adapter-git`. Five of the seven suites are files and two are the lib target's own pair
# (a unittest binary and a doc-test binary, which every `src/lib.rs` adds: that is why the suite
# count moves by seven for five files):
#   +4  `crates/gx-adapter-git/tests/ac_050.rs`         — AC-050's two cases (commit-operation delta /
#       branch-operation delta) with HEAD and the tree hash read through **gitoxide** rather than through (sem: SEM-tools-118)
#       the adapter, plus the retry that moves neither number and the entry read back off the
#       object database;
#   +6  `crates/gx-adapter-git/tests/git_conformance.rs` — the 7 contracts and the 8 laws, the
#       fixture cost beside the fs adapter's (M4H3-9's second number), and the measurement that the
#       shared harness **did not move** for a second adapter;
#   +11 `crates/gx-adapter-git/tests/git_delta.rs`      — the grammar (monoid, three refusals in
#       three words, a forged `kind`), the locator's clauses, reservation 6's `ScopeTooLong` road, and the (sem: SEM-tools-119)
#       probe that closed battery point (f)'s survivor: **nothing in 51 §7 or the L-list compared a
#       `postcondition` with a `precondition`**, so a scope that differed between `apply` and
#       `precondition` was invisible to all fifteen obligations;
#   +8  `crates/gx-adapter-git/tests/git_commutation.rs` — AC-052's git pair (4/6 after this hand),
#       the two-files-on-one-branch case that decides the scope, L6, and the two refusals
#       (`ForeignDelta`, `LocatorMismatch`) plus the unborn-branch `Ok(None)`;
#   +2  `crates/gx-adapter-git/tests/git_plan_purity.rs` — `plan` names no gitoxide call **and**
#       leaves `.git` byte-identical;
#   +3  the crate's own `src/locator.rs` unit tests (idempotence, clause 2, the scope).
# 🔴 **M7 hand 2** (req/98 §7-2, FR-028's git half / AC-074 / G-4). **+9 probes over +1 suite**: (sem: SEM-tools-120)
#   +9  `crates/gx-gate/tests/ac_074.rs` — the shipped **git** pack's conformance table (AC-074's
#       git half: 1 Admit, 2 Deny by statement, 1 Deny by no policy, the anchoring case and the
#       order-2 row), the boundary probe that keeps every case's locator inside what
#       `gx-adapter-git` can produce (`MAX_PATH_DEPTH` is 1, so a rule about a nested path could
#       never fire — req/99 §3 D-4), and **G-4**'s three: origin-blindness (the same bytes by both
#       roads give the same report), a third-party pack deciding by its own statements, and the
#       source scan of the runner region for the words an origin branch has to be spelled with.
#   +0  `crates/gx-gate/tests/pack_embedding.rs` — the road count moved from "one" to "one per
#       declared pack" and gained the equality with `SHIPPED_PACKS`; a rewritten claim moves no (sem: SEM-tools-121)
#       number, which is why it is stated here.
# 🔴 **M7 hand 2**'s remainder (FR-M7-3 key rotation, retroactive revocation / FR-M7-4 keyid's writer). **+22 probes over +2 (sem: SEM-tools-122)
# suites**:
#   +12 `crates/gx-witness/tests/revocation.rs` — the revocation entry's signature convention
#       (self-signed, two lines of defense on the producer side and the verifier side), the
#       ledger's monotonicity (the earliest wins) and how an entry under another key is handled,
#       every combination of the invariant's 4 inputs, the consistency and ordering of the 2
#       settings, and **an empirical measurement of the limit**: `issued_at` is outside the
#       signature (E-M2-6), so the default setting cannot detect a backdate (45 §3 TH-5's
#       residual "without a TSA, third-party proof of the revocation time is weak").
#   +10 `crates/gx-cli/tests/key_lifecycle_cli.rs` — the value `gx key gen --record` writes is
#       read by `gx serve`'s reader (M6H7-8's closure); without `--record` the project is
#       untouched; revoke/rotate; `gx receipt verify --revocations`'s 0/7 and `checks.revocation`'s 5 words (`not_consulted` and `not_revoked` are not given the same face); and **6=undetected** -- 44 §1.2's key section cannot name a "missing key" at `gen|list`, and only reaches it for the first time at `revoke` (E-M6-24's reading; cited in `SPEC_44_EXIT_ADDITIONS`).
# 🔴 **M7 hand 3** (req/98 §7-2 hand 3, FR-046 / AC-051 / AC-052 6/6 + §58 R-4). **+39 probes over +7 (sem: SEM-tools-123)
# suites** -- the fourteenth member, `gx-adapter-mcp`. Five of the seven suites are files and two are
# the lib target's own pair (a unittest binary and a doc-test binary, which every `src/lib.rs` adds:
# that is why the suite count moves by seven for five files, as it did for the git adapter):
#   +7  `crates/gx-adapter-mcp/tests/ac_051.rs` — AC-051's five derivations. D-1 the compiler
#       (`ToolCall`/`Admitted` unbuildable outside the crate, two `compile_fail` doctests with a
#       control), D-2 the mints derived by walking `src/` (2, both in `apply.rs`), D-3 the seven
#       methods **read out of the trait** with exactly one reaching a transport, D-4 Rule 2's single (sem: SEM-tools-124)
#       `adapter.apply` site derived over `crates/*/src` (the `publish = false` harness excluded from
#       the manifest, not by name), and D-5 the integration test: a denying gate leaves the server at 0 calls (sem: SEM-tools-125)
#       and an admitting one at exactly 1, through submit→verify→canonicalize→commit. The seventh
#       probe is the battery's doing: mutation (f) removed `apply`'s `ForeignDelta` check and nothing
#       went red, because the only foreign-delta probe was on `commutation`.
#   +7  `crates/gx-adapter-mcp/tests/mcp_conformance.rs` — 51 §7's seven and the nine laws over the
#       third adapter (**16 obligations**, K2 included), the non-vacuity of the fixture (the server was
#       reached, the catalogue is not empty), **the retry that sends one call** (mutation (g)'s
#       doing: on an effect substrate "the state did not move" is not "no second effect happened"), (sem: SEM-tools-126)
#       the third `FIXTURE_IMPL` number, and the harness scan widened to this adapter's vocabulary.
#   +11 `crates/gx-adapter-mcp/tests/mcp_delta.rs` — the locator's four refusals, reservation 6's eliding (sem: SEM-tools-127)
#       road, the two words of `decode` (`Unimplemented` vs `PayloadUnreadable`), associativity, the
#       forward ceiling, **the escrow ceiling that git could not have** (req/99 §3 D-4's other side),
#       R-3's per-adapter re-reading, and the `Ok(None)` of a tool with no declared restore.
#   +5  `crates/gx-adapter-mcp/tests/mcp_commutation.rs` — AC-052's mcp third (4/6 → **6/6**), the
#       case the design decides (two resources on one server **conflict**), L6, and the fact that
#       deciding independence reaches no server.
#   +3  `crates/gx-adapter-mcp/tests/mcp_plan_purity.rs` — `plan` reaches neither a call nor a read,
#       measured by a **counter** rather than by a byte comparison (which is stronger than either of
#       the other two adapters can manage: a read leaves no trace on a filesystem).
#   +3  `crates/gx-adapter-mcp/src/locator.rs` (unit) — L7's idempotence over five spellings, one
#       assertion per RFC 3986 §6.2.2 clause, and the residue §6.2.3 leaves as a **test**.
#   +3  doc-tests — the two `compile_fail` refusals and their control.
#   +0  `crates/gx-substrate-conformance/src/laws.rs` — **K2** (§58 R-4) adds an obligation, not a
#       test function: the count of `#[test]`s does not move and the count of obligations goes 15→16.
# 🔴 **M7 hand 4** (req/98 §7-2 hand 4: FR-028's mcp half / AC-052 6/6 / H-2 / H-9 / whether `Report::print`
# lives or dies + `req/38` §60's **R-9 counter-ruling**). **+26 probes over +5 suites**: (sem: SEM-tools-128)
#   +5  `crates/gx-gate/tests/shipped_set.rs` — the obligation that arrives with a **set**: the
#       composition is every declared statement and no other, the **locality** claim (every request
#       is answered by the composed set exactly as its own pack answers it — same arm, same deciding
#       ids), the non-vacuity gate (all six declared statements decide a row), Cedar's third rule for
#       a substrate no pack speaks for, and **AC-025's ids unmoved by the composition** (req/101
#       §9-1 raised the last one: the id reaches a receipt's CID, 42 §1.3).
#   +4  `crates/gx-gate/tests/ac_074.rs` — AC-074's **mcp** half ("pack-unit ×2"'s second pack): the (sem: SEM-tools-129)
#       seven-row table (1 Admit, 2 Deny by statement, the over-refusal guard, the `#`-anchor case,
#       the cross-substrate row and the order-2 row), its arithmetic, the parse-against-the-embedded
#       -pack probe, and the reachability boundary — which on this substrate bites hardest, because
#       the rule a reader expects ("forbid `shell.exec`") names the **tool** and a tool's name lives (sem: SEM-tools-130)
#       in the payload P-6 keeps opaque (D-4, the third time).
#   +1  `crates/gx-gate/tests/pack_embedding.rs` — the declared `substrate` of each pack is the one
#       its statements scope on, read out of the pack's own text.
#   +2  `crates/gx-gate/tests/false_admit.rs` — **H-9** both halves: a vector does not outlive the
#       absence that made it true (expiry, derived from `SHIPPED_PACKS`), and every shipped pack is
#       refused by a vector of the suite (tied to pack shipping). The suite's own gate is now the (sem: SEM-tools-131)
#       composed set and its vector count is 8 → **10** (FA-9 git, FA-10 mcp), which moves no probe
#       count because the vectors are data.
#   +6  `crates/gx-cli/tests/defaults.rs` — the **pair** §60 ruled: the default policy set is every
#       shipped pack, the default registry is the declared set, a `--substrate git` intent reaches
#       the git adapter's own refusal rather than an empty registry, an `--substrate mcp` intent is
#       refused **by the registry** (the half this hand leaves open), the deferral names its firing
#       condition (D-7), and — the sixth, which the battery asked for — a git change driven through
#       an engine `open_engine` built is **Admitted**, which is the only probe in this hand that
#       measures the branch rather than the function it calls (mutation (e) survived without it).
#   +3  `crates/gx-adapter-git/tests/h2_normalised_before_the_gate.rs` — **H-2** for git: `snapshot`
#       reports the normal form over four spellings, the shipped pack refuses all four while **all
#       four evade it raw**, and the gate normalises nothing ("in-gate normalisation is not adopted", measured by (sem: SEM-tools-132)
#       watching a raw spelling be admitted).
#   +3  `crates/gx-adapter-mcp/tests/h2_normalised_before_the_gate.rs` — the same three for mcp, where
#       two of the four spellings evade raw and the pair `file:///srv/../etc/passwd` /
#       `file:///%65tc/passwd` is the fs pack's known false admit **closed** by an adapter that
#       normalises.
#   +2  `crates/gx-substrate-conformance/tests/print_consumers.rs` — ruling #17 answered once, as a (sem: SEM-tools-133)
#       count: 16 call sites over 6 files, all three adapters among them, so `Report::print` lives
#       and no retirement mark is struck. Armed the other way too — the day the printing stops, the
#       question re-opens in red.
# 🔴 **M7 hand 5** (req/98 §7-2, additional ruling a = **FR-M7-2**). **+9 probes over +1 suite** -- hand 5's
# other 3 items (the AC-067 bench, disjoint-lock measurement, `cargo mutants --list` population
# re-measurement) are **measurements**, not probes, so they move the floor by not even 1. This is the 3rd time a bench has produced no `test result:` line.
#   +5  `crates/gx-log/tests/incremental_inclusion.rs` -- that after option A the proof matches **an
#       independent oracle transcribed from RFC 6962 §2.1.1** across every index (12 sizes, 2,329 of
#       them), that it also matches against the prefix tree (= the proof against a passed checkpoint)
#       with `verify_inclusion_of` passing without changing a single line, that the wire stays exactly
#       the 3 fields `{leaf_index, tree_size, audit_path}`, that only a completed tile is cached and each value is that tile's fold, and that append does not move an already-issued proof.
#   +4  `crates/gx-log/src/tile.rs`'s unit test -- 4 that the cache is **authoritative**: the folding
#       matches `mth`'s recursion (every prefix), the audit path matches the old implementation kept in
#       `#[cfg(test)]` (every index), only a completed tile is cached, and an **unaligned range** folds
#       from the leaves. The old `audit_path` was kept on the test side rather than deleted, because the
#       reimplementation's oracle is "code that used to be believed" (no-delete and "don't ship two roads"
#       both held at once). The 4th is a mutation-battery point (b)'s first survivor, **closed without classifying it**, for the reason in req/103 §7-3.
# 🔴 **M7 hand 6** (req/98 §7-2 hand 6 = **FR-M04** option A, and §62 **R-7**'s wiring). **+18 probes over
# +3 suites**. R-7's wiring itself (the 3 benches into `tools/ci.sh` stage 10) is **measurement wiring**,
# not a probe, so it does not move the floor -- what moves it is the doubt probe on the side that **holds
# the wiring as a declaration** (the 4th time a bench has produced no `test result:` line).
#   +11 `crates/gx-engine/tests/ac_vc.rs` -- FR-M04's 5 (AC-VC-1 the recomputed tally matches / AC-VC-2
#       detecting under-reporting (**even when the lie is signed**) / AC-VC-3 the signature core's 4
#       fields and the timestamp that is not covered / AC-VC-4 payload_type separation in both
#       directions / AC-VC-5 the 3 shapes of a gap = hole, truncated tail, **producing nothing at all**),
#       plus the Escalate bucket, **the 4th bucket unique to 43 T-4e** (an admission the gate never ran
#       does not fold into `admit`), the window's continuity across a restart, an empty window not becoming a repeat, and 🔴 **2 that measure ruling #3/#14's two limits in the shape of "the detection does not fire"** (policy relaxation / split view).
#   +4  `crates/gx-log/tests/verdict_checkpoint_store.rs` -- a parallel append-only file's round trip,
#       a torn tail's removal and **report**, the sequence number after recovery, and that the writer only ever `append`s.
#   +3  `probes/doubt/tests/bench_gate_doubt.rs` -- §62 R-7's **declaration**: that the 3 benches are in
#       stage 10, that the stage stays off by default, that each bench takes its threshold from env,
#       prints its provenance, and **exits non-zero**. The empirical measurement of the exit path itself is `tools/verify_m7h6.sh 4` (gotcha55's division of labor).
# 🔴 **M7 fix batch** (req/38 §64's launch, req/105's filed A-2/A-5/A-8). **+1 probe over +0 suites**.
#   +1  `crates/gx-cli/tests/defaults.rs` -- filed A-2: 1 that **derives** the set difference of "a
#       pack shipped but never once evaluated" (`SHIPPED_PACKS`'s substrate set minus the set actually
#       registered). Today's answer `{mcp}` was already pinned **by name**, but a name does not catch a 4th one.
#   +0  filed A-5 (`crates/gx-canon/tests/authority_boundary.rs`) is an assert added to an existing
#       probe = cross-checking the walked member count against the ledger. Filed A-8 (fs/git
#       conformance) is a 2-line rename. Neither adds a probe, so neither moves the floor -- **only the moved part is the +1 above**.
# 🔴 **P3** (req/119, req/38 §71's implementation lane). **+32 probes over +12 suites**. The count is the (sem: SEM-tools-134)
# reconstruction `probes/doubt/tests/floor_doubt.rs` F-1 prints, not an arithmetic somebody did by
# hand: the tripwire fired on this lane's first full run (1370/247 declared, 1402/259 held) and
# "both are fixed in the same place" is its own instruction.
#   +9  `crates/gx-mcp-wire/tests/` — wire_handshake (AC-P3-1/2 cross-checked against the revision
#       pin), method_classification (AC-P3-3's **derivation** + sweeping out the 12 unclassified +
#       each row's reason), raw_jsonrpc (AC-P3-4's 7 hand-written frames + control + ruling ③'s
#       shape), config_adoption (AC-P3-5's B-1, and putting the still-open B-2 in the report),
#       declared_limits (AC-P3-7 = B-9/B-10 passing **exactly as declared**, and the server->client-direction refusal).
#   +4  `crates/gx-api/tests/verdict_checkpoints.rs` — AC-P3-12 (201 / listing / single / 404, 44 §2.7's
#       limit refusal, the Bearer guard's walk).
#   +4  `crates/gx-cli/tests/verdict_checkpoint_surface.rs` — AC-P3-10/11 (issue->verify; a 1-bit
#       tamper exits 7; **recount catches under-reporting even without a key**; BehindTheLedger
#       against a signed head; a match between the 2 crates on origin).
#   +2  `crates/gx-mcp-wire/src/client.rs`'s 2 doc-tests — AC-P3-6: a `compile_fail` re-run of D-1
#       (`ToolCall` cannot be built from outside the crate) **with the wire folded into the graph**, and its control.
#       The floor counts doc-tests as well as `#[test]`, so these 2 also move the floor.
#   +5  `crates/gx-cli/tests/otel_export.rs` — AC-P3-15 (the off-by-default word, OTLP/JSON to a sink,
#       the locator being elided, **an unwritable sink changing nothing**, the named refusal of an HTTP endpoint).
#   +0  `crates/gx-cli/tests/defaults.rs` has 3 tests that **moved** (discipline 55②: a deferral's doc and
#       test that fired share the same window). The count does not move.
#   ⚠ `journal_changelog_doubt` is not this lane's addition (it is v0.2.5 batch's suite). EXPECTED_SUITES
#       was missing it -- this lane's own run discovered that and added it in the same window -- already reported to req/120.
# 🔴 **P3 adversarial audit lane** (req/125, req/38 §74's finding, v0.2.6 adoption lane item 8's
# wiring). **+10 probes over +5 suites**. The audit lane actually drove the lines (A-2/A-3/A-4/A-6)
# that req/120 had only written as "undriven", and added 5 new files with 4 substantive findings along
# the way, but this floor update by itself did not touch an existing file (the audit lane's own
# constraint = new files only), so `probes/doubt/tests/floor_doubt.rs`'s f1/f2 stayed red -- closing
# that red is this item's job. The numbers are taken from `floor_doubt`'s own `FLOOR_RECONSTRUCTED` measurement (`FLOOR_DECLARED=1404/259 FLOOR_RECONSTRUCTED=1414/264`).
#   +2  `crates/gx-mcp-wire/tests/audit_p3_a1_b2_agent_bypass.rs` — an empirical measurement of A-1's
#       declared row (B-2): that an agent bypassing the proxy reaches the server (a structural absence gx knows nothing about at all), and its control.
#   +3  `crates/gx-cli/tests/audit_p3_a2_fail_posture.rs` — an empirical measurement of A-2's two
#       FailClosed/FailOpen branches, and the finding that `gx wrap` has no flag that reaches FailOpen (gotcha66).
#   +2  `crates/gx-cli/tests/audit_p3_a3_concurrency.rs` — A-3's concurrency control (the same
#       resource), and the finding that the footprint=server claim does not reach the engine across different resources (gotcha64).
#   +2  `crates/gx-mcp-wire/tests/audit_p3_a4_crash_retry.rs` — A-4: a resend (arrival 2) from
#       constructing a second `McpAdapter` with no log, and the control of applying the same adapter twice (arrival 1).
#   +1  `crates/gx-cli/tests/audit_p3_a6_record_only.rs` — A-6: an empirical measurement of the
#       record-only path against a real Deny target locator (as usual, Deny -> canonicalize passes -> 1 arrival at the real server).
# 🔴 **P3.1 repair item①** (req/38 §74, gotcha65's repair, req/127). **+1 probe over +0 suites**:
#   +1  `crates/gx-cli/tests/ac_056.rs::idle_with_no_signal_outlives_the_grace_period` — that `gx
#       serve` does not self-terminate through a real 15-second wait (no connection, no signal) (before
#       the repair, `tokio::time::timeout` wrapped the whole `serve` call and terminated at 10 seconds
#       even with no signal), and that the real SIGTERM after it completes with `deadline_exceeded=false`/`exit=0`. The suite stays the existing `ac_056` (+0).
# 🔴 **P3.1 repair item②** (req/38 §74, gotcha66's repair, req/127). **+1 probe over +0 suites**: (sem: SEM-tools-135)
#   +1  `crates/gx-cli/tests/audit_p3_a2_fail_posture.rs::a2_cli_wired_through_open_engine_wired_failopen_reaches_a_committed_receipt`
#       — that `gx_cli::session::open_engine_wired`, which both `gx wrap --fail-posture open` and
#       `gx serve --fail-posture open` pass through, actually drives with `FailPosture::FailOpen` +
#       `UnreachableEvidence` and reaches a receipt stamped `enforced=false`/`fail_posture_engaged=true`.
#       The existing 3 tests were also revised to match the new behaviour (C-1: the function name is
#       unchanged). The suite stays the existing `audit_p3_a2_fail_posture` (+0).
# 🔴 **P3.1 repair, riding along** (req/120 §5/§8's residue, req/127). **+1 probe over +0 suites**: (sem: SEM-tools-136)
#   +1  `crates/gx-cli/tests/verdict_checkpoint_surface.rs::ac_p3_11_ac_vc_4_a_ledger_heads_signature_does_not_verify_a_verdict_checkpoint`
#       — re-runs AC-VC-4 (payload_type separation) through the CLI surface rather than directly
#       through the engine: transplanting the signature `gx log checkpoint` signed into `gx
#       verdict-checkpoint issue`'s output makes `gx verdict-checkpoint verify` refuse with
#       `checks.signature=false`/exit 7. The re-run of D-2/D-3/D-4 under the wire only empirically
#       measured `cargo test -p gx-adapter-mcp --test ac_051`'s existing dynamic scan (files=114
#       now includes gx-mcp-wire's 7 files, apply_sites stays 1 = unchanged), so the probe count does not move. The suite stays the existing `verdict_checkpoint_surface` (+0).
# 🔴 **P1** (`req/115` §A, `req/38` §76's leftover-work lane that started after P3's DoD closed
# and that `req/128b` carries this floor update for). **+33 probes over +7 suites** -- the 16th
# workspace member, `gx-adapter-postgres` (the 3rd `SubstrateAdapter`). The integration test has 5
# new suite files, but a new member with `src/lib.rs` always adds 2 more, a unittest line and a
# doc-test line (the "even an empty crate gets 2 lines" shape, ever since M2 hand 1). This crate's doc
# fences are all 3 tagged `` ```text `` prose, so doc-tests stay at 0 -- what moves is only the suite count. 5+2=7 suite, 18 unit (`src/{delta,locator,sql}.rs`'s `#[cfg(test)]`) +15 (sem: SEM-tools-137)
# integration(5 file)=33 probe:
#   +3  `crates/gx-adapter-postgres/tests/ac_p1_1_escrow_apply_offline_verify.rs` — AC-P1-1: 3 kinds
#       of DML's escrow (=`plan`) -> apply -> offline re-verification (a match between an independently-taken second `snapshot` and the digest). (sem: SEM-tools-138)
#   +3  `crates/gx-adapter-postgres/tests/ac_p1_2_undo_round_trip.rs` — AC-P1-2: plan→invert(T-10b
#       order) -> apply(forward) -> apply(inverse) -> a machine cross-check of the value's full recovery via `Fingerprint::cas_eq`.
#   +6  `crates/gx-adapter-postgres/tests/ac_p1_3_scope_out.rs` — AC-P1-3: out of scope (DDL /
#       multi-statement / without WHERE / a table with no PK / a multi-row insert) 5 of them + 1 direct
#       fail-open measurement, all through `Error::NotPlannable`/`Error::Unreadable`, a third path at plan/snapshot time (a correction of gotcha64's own self-application).
#   +2  `crates/gx-adapter-postgres/tests/ac_p1_5_concurrent_write_cas.rs` — AC-P1-5: that a direct
#       write from a separate connection moves `precondition`'s `cas_eq` to false (the main check + control). (sem: SEM-tools-139)
#   +1  `crates/gx-adapter-postgres/tests/pg_conformance.rs` — AC-P1-4: conformance harness 16/16
#       (the same denominator as adapter-git/adapter-mcp).
#   +18 `crates/gx-adapter-postgres/src/{delta,locator,row,sql}.rs`'s `#[cfg(test)]` (3+4+3+8) —
#       unit tests of the grammar / locator normalisation / digest / SQL scope determination (these ride on this crate's lib-unittest line). (sem: SEM-tools-140)
# 2026-08-14, M9 P2 (`req/130`/`req/131`): 1450 -> **1468** over 271 -> **274**, measured by
# `probes/doubt/tests/floor_doubt.rs`'s own reconstruction (`FLOOR_RECONSTRUCTED=1468/274`, not
# hand-counted). +18 probes / +3 suites, all new files: `crates/gx-witness/tests/
# ac_p2_3_key_encryption.rs` (10, AC-P2-3's round trip/wrong-passphrase/backward-compat/tamper
# tests), `crates/gx-cli/tests/secret_scan.rs` (2, NFR-012's dev-tree gate + AC-P2-4's fail-open
# self-check), `probes/doubt/tests/p2_auth_doubt.rs` (4, AC-P2-1's doc-conformance probe), plus 2
# more probes on an existing suite's line (`crates/gx-api/tests/auth.rs`'s equal-length-wrong-token
# and valid-token-200 cases, AC-P2-2) that do not add a suite of their own.
# 2026-08-14, M9 P4 (`req/132` §5 item 1): 1468 -> **1473** over 274 -> **276**. `sdk/wasm-verify`
# is the seventeenth workspace member and has no `tests/` directory (all five probes are
# `#[cfg(test)] mod tests` inside `src/lib.rs`, EXPECTED_SUITES below names integration-test
# *files* only and needs no new entry): +5 `#[test]` functions (the crate's own unit tests, none
# of the three doc-comment fences are ```` ``` ````-tagged so +0 doc-tests) and +2 suites (one
# unittest binary, one doc-test binary -- the same "even an empty crate gets 2 lines" shape every `src/lib.rs` (sem: SEM-tools-141)
# member adds, per `floor_doubt.rs::suite_targets`).
# M9 cross-cutting adversarial audit (req/136, included at the §81 acceptance window): +18 `#[test]` functions / +4 integration suites —
#   +9 `crates/gx-adapter-postgres/tests/audit_m9_p1_sql_attack.rs` (the SQL-attack corpus, fail-open 0/5)
#   +3 `crates/gx-adapter-postgres/tests/audit_m9_p1_locator_attack.rs` (the locator layer, an empirical measurement of gotcha78)
#   +2 `crates/gx-adapter-postgres/tests/audit_m9_p1_db_attack.rs` (real-DB bind values verbatim + true concurrency CAS)
#   +4 `crates/gx-witness/tests/audit_m9_p2_key_tamper.rs` (every offset's 1-byte flip / truncation / an equal-length wrong passphrase)
# doc-tests +0. The audit lane itself, under the frozen-instrument no-touch discipline, is left uncounted (req/136 §3), and the counting
# was carried out at Fable's acceptance window (req/38 §81) — floor_doubt's f1/f2 went RED and mechanically detected this omission.
# 2026-08-14, v0.2.7 Lane A (`req/38` §81 ruling 1, `req/136` §4-1 gotcha75, `req/137` §A1 item 2/6): (sem: SEM-tools-142)
# 1494 -> **1497** over 282 -> **284**. `PostgresAdapter` was registered into
# `crates/gx-cli/src/session.rs::register_default_adapters_with` (the fourth default adapter), and
# these two new files are the engine-via measurement AC-A2 asks for -- neither exists in (sem: SEM-tools-143)
# `crates/gx-adapter-postgres/tests/` (which never opens an `Engine`) nor in the M9 audit lane's own
# frozen instruments (`req/136` §3, not included by that lane's own no-delete regime): (sem: SEM-tools-144)
#   +2 `crates/gx-cli/tests/postgres_wired.rs` -- escrow(T-2)→gate(T-3/T-4)→commit(T-8..T-11)→
#       journal→receipt, in-process through the `Engine` this binary opened (positive), plus the
#       negative half: an alias with no DSN refuses by name and never `Ok` (fail-open 0).
#   +1 `crates/gx-cli/tests/postgres_db_e2e.rs` -- the same loop through the compiled `gx` binary as
#       four subprocesses (`submit`→`plan`→`verify`→`commit`→`undo`) plus two independent offline
#       receipt verifications, req/137 §A1 item 3's AC-A3.
# doc-tests +0 (`gx-cli` already has its unittest/doc-test suite lines counted; these are two more
# integration-test *files*, not a new workspace member).
# 2026-08-14, v0.2.7 Lane B (`req/137` §B1 item 4, commit fd4d43b, `req/139`): 1497 -> **1499** over
# 284 -> **285**. `req/137` §4 ruling 1 puts the floor-number surface in Lane A's hands alone ("the
# collision surface is only EXPECTED_SUITES/the README's floor numbers -- A holds it, B does not touch it"), so Lane B's own one new file is folded (sem: SEM-tools-145)
# in here rather than by that lane: +2 `crates/gx-log/tests/nfr_027.rs` (NFR-027's 180-day floor,
# `gx_log::NFR_027_MINIMUM_RETENTION_DAYS` — a compile-time `const` assertion plus a doc-conformance
# read-back of 33/35's own text) and +1 suite. Everything else in that lane's commit is a doc/
# comment change (32/33/34/35 canon text, `sql.rs`'s crate doc, `ci.yml`'s scope, three fmt fixes)
# and adds no probe.
# 2026-08-14, M8-c (`req/142` §1 M8-c item 1, `req/145_M8C_IMPL_REPORT_2026-08-14.md`): 1499 -> **1504**
# over 285 -> **288**. Three thin JSONL-generator integration-test files, one `#[test]` function per
# conformance kind they draw (`proptest::test_runner::TestRunner` used directly, not the `proptest!`
# macro, per the C-4 ruling `req/143` §2): +2 `crates/gx-canon/tests/conformance_gen.rs`
# (`canon_idempotence`, `repr_independence`) +2 `crates/gx-gate/tests/gate_conformance_gen.rs`
# (`admissibility`, `invariant_composition`) +1 `crates/gx-witness/tests/witness_conformance_gen.rs`
# (`receipt_verify`) = +5 `#[test]` functions / +3 suites. doc-tests +0. The `tests/support/mod.rs`
# each of the two new crates gained is a module of the binary beside it, not a target of its own
# (same shape `gx-canon/tests/support/mod.rs` already documents), so it raises neither count.
# 2026-08-14, v0.2-a A1-1 (`req/150` §A1-1, `req/151`'s target pick, `req/152`): 1504 -> **1508**
# over 288 -> **289**. +1 `crates/gx-adapter-mcp/tests/github_target_catalogue.rs` (4 `#[test]`
# functions): the github/github-mcp-server target's do/undo `Catalogue` (4 pairs) and its
# `fixtures/github-target.cedar` example pack (1 admit + 2 deny conformance cases, `check_pack`).
# doc-tests +0.
# 2026-08-14, v0.2-a A2 (`req/38` §92 ruling 1, tool-aware invert, `req/153`): 1508 -> **1520** over (sem: SEM-tools-146)
# 289 -> **290**. +1 `crates/gx-adapter-mcp/tests/mcp_restore_template.rs` (12 `#[test]` functions):
# the `RestoreTemplate` vocabulary (git-blob-sha derivation held to `git hash-object`'s vectors,
# resolution/refusal cases), `Catalogue::from_json` (one reader of the `--restore-catalogue` /
# `--mcp-restore-catalogue` file format, held against `fixtures/github-restore-catalogue.json`),
# catalogue-driven `invert` (template form + unchanged legacy `{contents, uri}` form + the third
# `Ok(None)`), and the `fixtures/github-target-a2.cedar` pack (1 admit + 2 deny, `check_pack`).
# doc-tests +0 (catalogue.rs's new fenced block is tagged `text`: `floor_doubt.rs::doc_tests`'s
# static reconstruction excludes exactly `text`/`ignore`, and cargo runs neither).
# 2026-08-14, v0.2-b (`req/150` §B AC-V2B-1, gotcha77 close, `req/155`): 1520 -> **1529** over
# 290 -> **290**. +9 `#[test]` functions, all in `crates/gx-cli/tests/secret_scan.rs` — an
# existing suite already named in EXPECTED_SUITES, so no suite-count change and no new entry:
# +8 per-rule probes for the v0.2-b expansion rules (each holds the corpus's positive line
#    verbatim plus the rule's *declared* non-catches as negatives, so a silently widened rule
#    turns a documented boundary red), and
# +1 `the_scanner_detects_all_eight_evasion_shapes_in_the_audit_corpus` — exact-equality (8
#    findings/8 rules) over the frozen M9 audit corpus `tools/audit_m9_p2_scanner_evasion_
#    fixture.sh` (untouched, 33 NFR-012 gotcha77's note "an audit record: do not touch"; the dev-tree gate now (sem: SEM-tools-147)
#    excludes it by path the way `secret_scan_positive/` is excluded — a corpus the scanner
#    provably sees is a positive fixture, not a leak).
# doc-tests +0.
# 2026-08-14, v0.2-c (`req/150` §C DR-46-2/DR-46-3, `req/156`): 1529 -> **1530** over 290 ->
# **290**. +1 `#[test]` function, `conformance_gen_step_bounded` in
# `crates/gx-gate/tests/gate_conformance_gen.rs` — an existing suite already named in
# EXPECTED_SUITES, so no suite-count change and no new entry. The test generates the sixth
# conformance kind (`step_bounded`, the DR-46-3 non-tautological declared class) and asserts
# both `law_holds` branches are non-empty in the batch (AC-V2C-1's distribution gate). The
# DR-46-2 changes (variable-length keys + `key_bytes` + real-encoder-derived rank order +
# corpus order-gap assertions) live inside the two existing gx-canon generator probes and add
# no new `#[test]`. doc-tests +0.
# 2026-08-15, v0.3-a A-2' (`req/38` §98 ruling 1 + §99 ruling, two-phase escrow, `req/162`): (sem: SEM-tools-148)
# 1530 -> **1543** over 290 -> **292**. +2 suites: `crates/gx-adapter-mcp/tests/do_result.rs`
# (8 `#[test]` functions: the /(\d+)$ derivation on the two real URL shapes + refusals, the
# partial escrow's construction/determinism, completion + the fail-safe folds, and the legacy
# raw-byte assert that a pending-free op encodes byte-identically to the pre-two-phase grammar)
# and `crates/gx-engine/tests/two_phase_escrow.rs` (5 `#[test]` functions: the Pending ->
# ApplyObserved -> InverseCompleted record order with undo running the completed inverse, the
# §99 ruling 2-④ fold with the commit continuing, both crash-window recoveries, and the (sem: SEM-tools-149)
# unregistered-registry control). doc-tests +0 (the transport doctest pair changed signature,
# not count).
# 2026-08-15, v0.3-a A-3 (`req/38` §98 ruling 2, settle pre-flight + --retry, `req/163`): (sem: SEM-tools-150)
# 1543 -> **1550** over 292 -> **293**. +1 suite: `crates/gx-cli/tests/undo_settle.rs`
# (7 `#[test]` functions: poll-1 match with the distribution line on stderr, --settle 0
# disabled, timeout on a third-party-moved world firing once anyway with the result unchanged,
# missing-receipt skip (fail-safe, immediate), the second-undo prompt-refusal guard (a launch
# whose answer is fixed is not polled), --retry not re-firing a denial, and the
# cfg(unix) --retry-on-ApplyFailed run whose three attempts are three journalled T_u.
# The unix cfg is why the floor counts it: the floor is measured under WSL2 (permanent row 84), where (sem: SEM-tools-151)
# it runs). `Engine::live_digest` itself is read-only and adds no engine-side probe; the
# lifecycle/main changes ride under the existing undo_cmd/exit suites. doc-tests +0.
# 2026-08-15, v0.3-a minor repair (`req/38` §102 ruling 1, F1, `req/165`): 1550 -> **1551** over 293 (sem: SEM-tools-152)
# (suites unchanged). +1 `#[test]` in `crates/gx-engine/tests/two_phase_escrow.rs`: the live
# `Pending` row vs `Engine::undo` behaviour probe that closes the F1 mutation survivor (the
# guard's refusal is now pinned by name; deleting it turns this probe RED). §102 ruling 2/3 are a (sem: SEM-tools-153)
# doc sentence (catalogue.rs) and a script gate (a3_settle_undo_e2e.sh) -- neither moves the
# floor. doc-tests +0.
# 2026-08-15, v0.3-b-1 K6 mutant-kill (`req/38` §73's priority 9, `req/159` §B-1, `req/167`): (sem: SEM-tools-154)
# 1551 -> **1557** over 293 (suites unchanged). +3 `#[test]` in
# `crates/gx-engine/tests/ac_vc.rs` (the four-bucket restart recount that kills the
# tally_from_the_journal survivors, and the two second-window subtraction probes for the
# escalate/unverdicted window arithmetic) and +3 in `crates/gx-engine/tests/crash_recovery.rs`
# (the T-4e mid-section resume, the half-filled-pair refusal at resume, and the E-M5-11 guard
# scan that pins commit's behaviourally-unreachable arm). Each is a targeted kill of a mutants
# run e survivor, verified by hand-injection (RED under the mutation, GREEN restored).
# doc-tests +0.
# 2026-08-15, v0.3-d (`req/159` §D items 5/6, `req/169`): 1557 -> **1561** over 293 (suites
# unchanged). +4 `#[test]` in `crates/gx-cli/tests/secret_scan.rs` — the three-shape v0.3-d
# corpus detection (exact equality: JWT / Azure connection-string key / GCP service-account
# marker, closing 33 NFR-012's residual clause behind its corpus precondition) and the three per-rule (sem: SEM-tools-155)
# positive/negative probes. `exit_matrix_cli.rs` gained `HAND_P3_EXITS` coverage by widening
# its existing five probes (GROUP_SECTIONS 13 -> 15, pending == `gx serve` alone) — +0 there,
# because the census tests moved rather than multiplied. doc-tests +0.
# 2026-08-15, v0.4-a NFR-011 close (`req/38` §109 ruling=DR-46-5 option (b), `req/172`): 1561 -> **1564** (sem: SEM-tools-156)
# over 293 (suites unchanged). +3 `#[test]`, one per existing suite: gx-core `m2_types.rs`
# (the JSON-face schema gate: `DsseSignature` serialises to exactly {keyid, sig} and never an
# `alg`), gx-canon `golden_vectors.rs` (the DAG-CBOR face: map header `a2` and the exact
# 22 spec-derived bytes, so a silently added field turns `a3` and RED), and gx-cli
# `key_surface.rs` (the C3 pin: `--alg`'s refusal names the single-sourced "ed25519" pin and
# repairs no near-miss spelling; the pin's substance is the `VerifyingKey` type itself).
# doc-tests +0.
# 2026-08-15, v0.4-e bundling the minor residues (`req/38` §110/§107's residue, `req/175`): 1564 -> **1570** (sem: SEM-tools-157)
# over 293 (suites unchanged). +6 `#[test]` across three existing suites: gx-core
# `m2_types.rs` +2 (the field census of the two gx-core signature carriers — `Checkpoint`
# exactly {origin, root_hash, signature, timestamp, tree_size} and `VerdictCheckpoint`
# exactly its eight FR-M04 keys with the four-bucket tally, each pinning the in-place
# `{keyid, sig}` of the signature it carries — the constructive form of 33 NFR-011 note 5's
# permanent wire-alg prohibition at the carriers, §110's declared residue), gx-witness (sem: SEM-tools-158)
# `receipt_verdict_wire.rs` +2 (the same census for `DsseEnvelope` exactly {payload,
# payload_type, signatures} with the base64 payload face and `Receipt` exactly {envelope,
# issued_at}, both built by the real producer `Receipt::issue`), and gx-cli `exit_map.rs`
# +2 (the wrap `--check-config` exit-7 live pair, §107 residue item 4: a residual direct entry and a (sem: SEM-tools-159)
# never-adopted entry both exit 7 with the residual named; adopt-then-check exits 0 — the
# last source-reading-only 7 in HAND_P3_EXITS now has a live measurement). doc-tests +0.
# 2026-08-15, v0.4-f K6 the remaining 20 (`req/38` §73's remaining survivors, `req/176`): 1570 -> **1578** over (sem: SEM-tools-160)
# 293 (suites unchanged). +8 `#[test]` across five existing gx-engine suites, each a targeted
# kill of a mutants run e survivor (verified by hand-injection: RED under the mutation, GREEN
# restored). Behavioural: `ac_vc.rs` +1 (the reopened checkpoint chain does not republish
# published escalations/T-4es — `Engine::open`'s published fold, 4 mutants), `subject_index.rs`
# +1 (a blocked row is refused after its blocker commits — verify's 43 §8 re-evaluation),
# `supersede.rs` +1 (an undo begins with its own journalled draft — undo's T-1 guard),
# `journal_roundtrip.rs` +1 (a record exactly at MAX_RECORD_BYTES replays — the inclusive
# ceiling). Scan (Λ4/M7 precedent, for the 8 equivalent survivors §176 §3 argues):
# `subject_index.rs` +1 (the re-seat subject comparison), `lifecycle_transitions.rs` +3 (the
# supersede guard's three disjunctions; InjectedEvidence::none's literal; the replay/open
# boundary trio). The five mutants already killed by suites outside run e's reduced detector
# (`test_packages=gx-engine` on the staging tree) get no new test — `req/176` §2 records which
# existing test catches each. doc-tests +0.
# 2026-08-15, v0.4-h a different kind of scanner corpus (`req/38` §107 residue item 6 / §113's residue, `req/178`): (sem: SEM-tools-161)
# 1578 -> **1582** over 293 (suites unchanged — the four new tests live in the existing
# `secret_scan` suite). +4 `#[test]` in `crates/gx-cli/tests/secret_scan.rs`: the v0.4-h
# three-shape corpus exact-match (`secret_scan_v04h/corpus.txt` — one finding per shape, by
# exactly the three new rules) and one per-rule probe each for the three issuer-documented
# formats the residual named (Entra ID client-app secret `7Q~`/`8Q~` identifiable signature,
# Google `AIza` API key, npm `npm_` access token; 16 rule families become 19). The shapes
# whose formats could not be collected from an issuer publication (pre-2021 AAD secrets,
# Google `AQ.` express keys) are declared unimplemented in the module doc rather than
# guessed at (`req/155`). doc-tests +0.
# 2026-08-15, v0.4-i 44 §2.2's response-shape census (`req/38` §115's residue / `req/177` §5, `req/179`): (sem: SEM-tools-162)
# 1582 -> **1589** over 293 -> **294**. +1 suite: `crates/gx-api/tests/wire_census.rs`
# (7 `#[test]` functions: the Admit pipeline's ten response shapes each pinned to exactly its
# key set through the real router — the commit/undo composites that mount riders beside the
# Receipt pair being the shapes `req/177` §5 declared unfixed; healthz + cancel + the
# `ProblemDetail` five-key census through three refusal roads (404/422/401, RFC 9457
# status-mirror and type-URI conformance included); the escalation composite on the E-M3-4
# ceiling fixture; the three verdict-checkpoint answers with tally/signature interiors; the
# stream envelope (44's four fields + M6H6-2's cursor) and all ten event `data` shapes over
# two fixtures, with a table-vs-`stream::EVENTS` denominator gate so the census cannot
# silently shrink). `json!` composites are not type-fixed (§113 ruling 4 covers derives only), (sem: SEM-tools-163)
# so these censuses are the first field-set gate on the HTTP layer itself — the type censuses
# of §110/§113 cover the carried values transitively and cannot see a handler that wraps or
# decorates them (`req/177` §2). doc-tests +0.
# 2026-08-15, v0.4-j the 4 signature-carrier structures' CBOR-face golden (`req/38` §113's residue / `req/175` §5-2, (sem: SEM-tools-164)
# `req/180`): 1589 -> **1593** over 294 (suites unchanged — the four new tests live in the two
# existing suites that already hold the JSON-face censuses' CBOR neighbours). +2 `#[test]` in
# `crates/gx-canon/tests/golden_vectors.rs` (`Checkpoint` five-entry map, `VerdictCheckpoint`
# eight-entry map incl. the FR-M04 all-refusals window) and +2 in `crates/gx-witness/tests/
# receipt_verdict_wire.rs` (`DsseEnvelope` three-entry map, `Receipt` two-entry map) — the
# gx-witness pair lives there because gx-canon cannot name gx-witness (dependency runs the
# other way). Every expected byte string is hand-derived from RFC 8949 §3 + 42 §2.1 (not
# transcribed from the encoder) and was doubled against an independent from-scratch emitter
# before first run; each test also asserts is_canonical/scan_strict and decode round-trip. The
# fixtures are the v0.4-e JSON census values, so each carrier's two faces pin one logical
# value. doc-tests +0.
# 2026-08-15, v0.4-k EXTENSION's 4-endpoint response-shape census + 44 §2.7's codification (`req/38` §117 ruling 6's residue /
# `req/179` §2's "outside the denominator", `req/181`): 1593 -> **1595** over 294 (suites unchanged — the two new (sem: SEM-tools-165)
# tests live in `crates/gx-api/tests/wire_census.rs`, the suite whose declared denominator they
# close). +2 `#[test]`: the three list pages (`GET /candidates`/`/escalations`/`/transformations`)
# each pinned to 44 §2.7's two-key `{items, next_cursor}` composite on both arms (empty and full),
# every row of every page pinned to its exact key set (4/7/6 keys, as 44 §2.7's v0.4-k addendum
# now writes them), and `GET /ledger/consistency` pinned **as shipped** (`{from, to, proof}` +
# 42 §3.11's proof interior) under 🔴 DR-44-1 (the wrapper disagrees with the CLI twin, the SDK
# return type and `GET /ledger/proof`; the census holds the wire still while the ruling is open).
# All bodies are `json!`/`Map::insert` composites in `list.rs`, so the real-router probe is the
# only field-set gate they have. doc-tests +0.
# 2026-08-15, v0.4-b Notion (second SaaS) catalogue declaration (`req/170` §3-B, `req/169` §8,
# `req/38` §107 ruling 7, `req/183`): 1595 -> **1603** over 294 -> **295**. +1 suite (sem: SEM-tools-166)
# `crates/gx-adapter-mcp/tests/notion_page_catalogue.rs` (8 `#[test]` functions: the pair
# `API-post-page` -> `API-delete-a-block {block_id: {do_result: "/id"}}` parsed from the fixture
# by the server's **real** tool names, the template being the fifth word alone (`resolve_split`
# resolves nothing / one pending member; one-phase `resolve` refuses by name), the string-only
# `Const` reason for the DELETE-tool pick (no boolean `in_trash` argument in the declaration),
# the partial escrow through `invert`, completion against the **real observed** `API-post-page`
# result (`fixtures/notion-post-page-observation.json`, captured from the real server), the
# no-`/id` folds, and the `notion-target-a2b.cedar` pack's parse + admit/deny rows). The gx-arm
# E2E (`tools/a2b_notion_undo_e2e.sh`) is **RED at plan** today (the server declares no
# `resources` capability; DR-V4B-1 in `req/183`) and is not part of this floor. doc-tests +0.
# 2026-08-15, semantics-migration pilot (`req/38` §121 ruling 3, `req/185`): 1603 -> **1608** over
# 295 -> **296**. +1 suite `probes/doubt/tests/cjk_doubt.rs` (5 `#[test]` functions: the CJK ratchet
# over `req/cjk_baseline.json` -- baseline shape, no directory grows, migrated directories are zero,
# the ledgers under `req/semantics/` account for every migrated line and every `(sem: SEM-...)`
# anchor resolves, and the classifier's positive/negative controls). The migration itself (gx-core
# 350 -> 0, sdk/typescript 51 -> 0 CJK lines) is doc-only and moves no probe. doc-tests +0.
# 2026-08-16, CJK content-fidelity probe (`req/38` §136, req/205): 1608 -> **1609**, suite count
# unchanged at **296** (a sixth `#[test]` added to the existing `probes/doubt/tests/cjk_doubt.rs`
# binary, not a new file). c4 proves the ledger's `at:` lines equal the baseline snapshot and that
# every id and every `(sem: SEM-...)` anchor name each other, but not that an anchor sits on the
# sentence its own ledger entry describes -- a uniform anchor-number shift within one file passes c4
# silently. c6 reads each verbatim (degrade field `VERBATIM`) entry's own gloss, takes its first six
# ASCII words, and requires them within eight lines of an occurrence of the entry's own anchor,
# unless that occurrence is a shared block-level citation for several ids at once. doc-tests +0.
# 2026-08-16, v0.4-l small repairs (`req/189`, `req/38` §122 A + §119 DR-44-1(a) + §123 DR-V4B-2/3):
# 1613 -> **1629** (+16 `#[test]`), 298 -> **302** suites (+4 files):
# `crates/gx-engine/tests/audit_v04l_engine_repairs.rs` (5: H-04 x2, H-05 x2, M-01),
# `crates/gx-api/tests/audit_v04l_api_repairs.rs` (5: H-04, H-10, H-11, M-14/L-04, M-15+DR-44-1),
# `crates/gx-adapter-mcp/tests/dr_v4b_2_const_json.rs` (3), `crates/gx-mcp-wire/tests/dr_v4b_3_strip.rs` (3).
# Existing suites changed but not counted: wire_census (ProblemDetail 3->9 roads, DR-44-1 bare, rows +3 keys),
# lists (DR-44-1 bare). ~~SDK e2e +1 (`node --test`, outside this floor)~~ -- 🔴 **R12**
# (`req/242` M-04): the struck words were true for four releases and are the reason the SDK's
# own census went stale unnoticed. Stage 5b runs `node --test` from the clone; its count is
# `MIN_SDK_PASS`, on its own line, because `MIN_PROBES` counts what cargo printed.
# + DR-V4B-3's `gx wrap` half (`crates/gx-cli/tests/dr_v4b_3_wrap_strip.rs`, 2): 1629 -> **1631**, 302 -> **303**.
# + DR-43-1(a)/DR-43-3's undo CAS and its refusal taxonomy (`req/38` §132 ruling 2 / §144
# ruling 2, `req/216`): `crates/gx-cli/tests/undo_cas_e2e.rs` (8) + `crates/gx-adapter-mcp/tests/
# undo_cas_mcp.rs` (2) + one census arm in `crates/gx-api/tests/wire_census.rs` (the 409
# ProblemDetail an undo takes when the world moved -- an existing suite, so no suite-count change):
# 1631 -> **1642**, 303 -> **305**.
# + R1b (req/219, DR-43-6/DR-43-7 repairs of req/215's H-01..H-05): +8 `#[test]` functions and +3
# suites -- `crates/gx-engine/tests/catch_up_eviction.rs` (2: the eviction rule driven by two engines
# over one project, which req/215 H-04 found had no test at all), `probes/doubt/tests/catch_up_scan.rs`
# (3: R1's own falsifier, "`match record` in the catch-up body means this design is dead", turned
# from prose into a scan), `crates/gx-cli/tests/serve_runtime_r1b.rs` (3: a ledger that moves under a
# live server, read verbs that no longer shorten a torn one, and a GET as new as the disk).
# The two lanes ran in parallel on disjoint regions and their floors add:
# 1642 -> **1650**, 305 -> **308**.
# + R2 (req/221, DR-43-2 lane R2: the draft archive, DR-43-4's reaper, req/38 §156 ruling 3's
# `.gx/` declarations and ruling 2's three): +6 `#[test]` functions and +1 suite --
# `crates/gx-cli/tests/serve_runtime_r2.rs` (4: a restarted server that undoes a row it never
# planned, the CAS still refusing over a moved world after that rebuild, a Candidate expired by the
# timer rather than by a caller, and /healthz refusing to call a disagreeing project healthy),
# `crates/gx-api/tests/wire_census.rs` (+1: BUSY's sixth ProblemDetail member, DR-43-5 (3)) and
# `crates/gx-cli/tests/gx_layout.rs` (+1: the two declared-but-untouched `.gx/` rows). Renames do
# not move the count: `undo_cas_e2e`'s H-16 probe is now
# `an_undo_of_an_undo_re_applies_the_original_change`, still one function.
# 1650 -> **1656**, 308 -> **309**.
# + R3 (req/223, req/38 §160 ruling 2: the repairs for req/222's H-01..H-06): +8 `#[test]`
# functions and +1 suite -- `crates/gx-cli/tests/serve_runtime_r3.rs`, one probe per accident the
# third adversarial audit measured (a deleted commit receipt refusing the undo instead of firing
# it; a commit whose receipt the archive would not take reported as a failure rather than a plain
# 200; a receipt copied from another transformation refused as evidence; a refused rebuild leaving
# the journal length where it was; a deadline surviving a restart; a same-length rewrite of the
# ledger stopping the writing; and a project whose two files disagree having a door, `gx repair`).
# Seven of the eight were **red on the parent commit** -- that is the point of them, and req/222
# §9 asked for four of them by name before they existed. The eighth is the self-kill on the fifth:
# a rewrite in the *middle* of a ledger is caught by the next write and not by a read, and the
# probe asserts the guarantee half while printing the denominator half rather than pinning it.
# Renames and inversions do not move the count:
# `undo_settle`'s skip probe is now `a_missing_commit_receipt_refuses_the_undo_rather_than_firing_it`,
# still one function.
# 1656 -> **1664**, 309 -> **310** on this lane's own parent.
#
# 🔴 The H-09 lane (req/224, GO condition 6) landed on main while R3 was running, and its floor
# was 1656 -> **1662**, 309 -> **310** over the same parent. The two lanes touched disjoint
# regions, so the floors add rather than replace: 1656 + 6 + 8 = **1670**, and 309 + 1 + 1 =
# **311**. Written out because a merge that silently kept one side's number would lower the
# floor by the other side's probes -- which is the one direction this file exists to prevent.
# + R4 (req/226, req/38 §163 ruling 2: the repairs for req/225's H-01..H-03, and §163 ruling 1's
# standing definition of done for a repair lane): +12 `#[test]` functions and +1 suite --
# `crates/gx-cli/tests/serve_runtime_r4.rs`. Seven of the twelve are the audit's three accidents
# (`gx repair` without `--yes` writing; the same report beside a live server taking /healthz from
# 200 to 500; `--yes` still repairing, which is the control that keeps the first two from passing
# on a no-op; an undo on a project whose engine key is not its actor key; a key rotation that used
# to kill the undo of every earlier commit; a same-length rewrite of the journal's last record;
# and a rewrite in the middle of it). The other six are this lane attacking its own new code from
# doors the first seven do not use, which §163 ruling 1 makes a standing requirement after four
# audits in a row found their highest item in the previous lane's repair: the whole `.gx/` tree
# rather than two files, a project with no journal rather than a damaged one, a journal that shrank
# rather than one rewritten at the same length, a key that is absent rather than wrong, and a
# server and a CLI writing in turn (the door where a byte-comparison detector fails by being too
# sensitive). **Ten of the twelve were red on the parent commit**; the two that were green are the
# `--yes` control and the co-residency guard, and both are loss conditions rather than
# reproductions.
# 1670 -> **1682**, 311 -> **312**.
# + R5 (req/228, req/38 §165 ruling 2: DR-43-9, the repair for req/227's H-01 and its M-01..M-07):
# +16 `#[test]` functions and +1 suite -- `crates/gx-cli/tests/serve_runtime_r5.rs`. Four are the
# audit's H-01 in its three shapes and its consequence (a record overwritten with the bytes of
# another record from the same file; two adjacent records swapped; one bit inside a payload; and
# the one that measures the disk rather than the report -- a replaced `Committed` record whose
# delta the next start-up re-applied, taking an operator's file from `three` back to `one`). Six
# are the audit's middle band (M-01 `journal_intact` that could not be false, M-02 the reader's
# door making two directories, M-03 the report that would not open on a read-only filesystem,
# M-04 the report that would not open a project missing its verdict chain, M-06 a key file whose
# name and contents disagree, M-07 a refusal pointing at a directory the deployment does not
# have). The other six are this lane attacking its own new code from doors the first ten do not
# use (§163 ruling 1): a journal whose chain is **recomputed end to end** so that it verifies
# perfectly, a journal in the old format with no chain at all and the same substitution on it, a
# record deleted with the file's length made up at the end, a server and a CLI writing in turn
# (the door where the new detector fails by being too sensitive -- and it now runs on reads as
# well as writes, so a false positive would take /healthz down for a healthy project), and the
# file this lane refuses to truncate. **Four of the sixteen were red on the parent commit** --
# the four that reproduce the audit; the rest are about code that did not exist there.
# 1682 -> **1698**, 312 -> **313**.
# + R6 (req/230, req/38 §167 ruling 2: DR-43-11 + DR-43-10 minimal, the repair for req/229's
# H-01/H-02 and its M-01..M-06): +16 `#[test]` functions and +1 suite --
# `crates/gx-cli/tests/serve_runtime_r6.rs` -- and +3 unit tests inside `crates/gx-log/src/head.rs`
# (an existing suite, so the suite count moves by one and not two). Two are the audit's H-01 in its
# two cuts (both files truncated at a frame boundary between a ledger entry and the record that
# closes it, where the recovery re-applied a delta and took an operator's file from `three` back to
# `two`; and the cut placed outside a commit, where the project came back healthy and the next
# commit signed a second root for a tree size it had already published). Two are H-02 (a journal
# stripped of its marker and links, accepted as `legacy` with no warning; and the audit-five rewrite
# re-run on such a file, live, with a signed checkpoint over it). Five are the audit's middle band
# (M-01 gx cutting 98% of a journal because eight bytes were removed, M-02 the report that would not
# open a project missing its ledger file, M-04 a `--yes` that trimmed the ledger to agree with a
# journal it had just called untrustworthy, M-05 `repaired` reporting its own argument, M-06
# `gx key list` calling a file healthy under a key id that exists nowhere). One is DR-43-10 (an
# exported head refusing a project that went backwards, with the removed commit's receipt `refuted`
# by the project and `verified` by the export). The other six are this lane attacking its own new
# code from doors the first ten do not use (§163 ruling 1): the recorded head **deleted** (s1_), an **older head this project itself signed** put back in place after a rollback (s1b_),
# the format declaration **deleted** (s2_) -- both of which assert that gx *passes* the project,
# because both repairs live inside the attacker's write scope and saying so is the point -- an
# edited export refused by the verifier that checks its signature (s3_), a server and a CLI writing
# in turn so that a monotonicity check cannot fail by being too sensitive (s4_), and a project that
# predates this release opening exactly as it did (s5_).
# **Two of the sixteen were red on the parent commit in this file's sense** -- the rest are about
# code, keys and verbs that did not exist there; the parent-commit raw for H-01 and H-02 is in
# `req/230` §1 and was taken with the audit's own probe harness rather than with this suite.
# 1698 -> **1717**, 313 -> **314**.
#
# DR-44-9 (`req/38` §168, `req/187` §5, `req/231`): the four HTTP read views the GUI session filed
# for. `crates/gx-api/tests/dr44_9_views.rs` is a new suite of **4** -- the adversarial half, one
# attack per addition (a rewritten world that must not move a signed receipt's decoded view, the
# document still reading back as a bare `Receipt`, the consistency judgement shown discriminating
# rather than being a literal `true`, and the two resolved verdict-window boundaries held against
# the `at` the verify answer carried). **1717 -> 1721, 314 -> 315** (rebased onto R6 at merge: this lane branched from 400156d, where the floor still read 1698/313).
# R9 (`req/38` §175 ruling 2, `req/236`): `crates/gx-cli/tests/model_a_probes.rs` gains **4**
# probes -- a fragment at a content address that must be neither `Available` nor adopted, the
# escrow census answering from a second process, a recovery run under the wrong key that leaves
# the row resumable, and a crash whose staging files `gx repair --yes` sweeps. Three of the four
# are red on the binary this lane started from. No new suite, so only the probe floor moves:
# **1746 -> 1750, 317 unchanged**.
#
# 🔴 **R10** (`req/238` H-01, `req/38` §177 ruling 2) adds three more to the same suite: a
# declaration that is **gone** is reported and is not written back in silence, settings that are
# gone do not come back as the shipping default, and a declaration that is not text keeps its
# bytes. All three are red on the binary this lane started from (R9, `0a5c864`) and none of them
# adds a suite: **1750 -> 1753, 317 unchanged**.
#
# 2026-08-17, R11 (`req/38` §179 ruling 2, `req/240`): `model_a_probes.rs` gains **five** more --
# the keyless `gx repair --yes`, the read-only repair, the project whose journal is gone, the two
# undeclared files (`.gx/.gitignore` + `*.pre-repair.<n>`) and the serving process that notices a
# declaration going missing under it -- and no suite is added: **1753 -> 1758, 317 unchanged**.
#
# 2026-08-17, R12 (`req/38` §181 ruling 2, `req/242`): `model_a_probes.rs` gains **five** more --
# a declaration whose version line is not a number (two shapes in one probe), a writer verb that
# must not re-arm a digest that fired, a journal that must not be re-created, a `gx repair --yes`
# that must print even when the engine will not open, and a `--yes` that cannot make `.gx/LOCK` --
# and `probes/doubt/tests/declaration_writer_doubt.rs` arrives with **five** more in a **new**
# suite, the call-site census that counts how many roads write a project's own declarations:
# from the source, and `crates/gx-cli/tests/limits_sync.rs` gains **one** -- the buyer-facing block
# the release wrote has to name its own gates (`req/242` L-09):
# and `model_a_probes.rs` gains a sixth gate -- the four shapes of gate (2), one of which was
# red on this lane's own first binary (`Layout::logged`):
# **1758 -> 1770 over 317 -> 318**.
#
# 2026-08-17, R13 (`req/38` §183 ruling 2, `req/244`): `declaration_writer_doubt.rs` gains **two**
# in the suite it already has -- D-6, which counts `println!` call sites in `crates/gx-cli/src`
# (`req/244` H-01: `println!` panics on a write error, so a verb that prints through it can write
# to a project and then die at exit 101 with nothing on stdout), and D-7, which asserts that the
# census's own write vocabulary is not narrower than the set of write APIs this workspace uses
# (`req/244` M-02: `fs::copy(` was already in `gx-engine/src/store.rs` and the census could not see
# it). Both are new `#[test]` functions in an existing suite, so the suite count does not move:
# **1770 -> 1775 over 318**.
#
# 2026-08-18, R14 (`req/38` §186 ruling 2, `req/246`): `model_a_probes.rs` gains **six** gates in
# the suite it already has, one per finding of the fourteenth audit -- a refusal whose stderr will
# not take it staying inside 44 §1.4's table (H-01: five arms, three runs each, exit **101** in all
# fifteen, the cheapest being `gx receipt show <missing> 2>/dev/full`), the journal-less road filing
# the record of what it wrote (M-01), one predicate answering "has this project been used" at both
# doors (M-02), a durable record that holds no generations and survives run 127 (M-03: 130 runs),
# a declared directory blocked by a file having a named refusal and an exit (M-04), and all four
# refusing roads creating no directory (L-01). New `#[test]` functions in existing suites, so the
# suite count does not move: **1775 -> 1781 over 318**.
#
# 🔴 **R15 / `req/259` H-01 / M-01 / L-01** -- five more, still in existing suites: four Model A
# gates (the exit of a verb whose answer rides stderr; a generated key named again from the store;
# every declared directory blocked by a file; the remedy being true of every one of them) and one
# census that counts 44 §2.3's stacked rows against `RULED_ADDITIONS` -- the third face of the
# refusal vocabulary, which had no machine and was 25/25/**24**. **1781 -> 1786 over 318**.
#
# 🔴 **R16 / `req/262` H-01 / M-01 / M-02** -- eight more, still in existing suites, so the suite
# count does not move. Five Model A gates (an HTTP answer that does not depend on the error stream;
# a blocked project getting no server, on all seven declared directories; every `gx ...` a refusal
# names being a command this binary has; a keyless repair naming the flag it needs; a refusal not
# asserting a cause it did not measure) and three unit tests on the two new typed roads to a
# standard stream (`gx-api/src/notes.rs` 1, `gx-mcp-wire/src/notes.rs` 2) -- which exist because
# the census's window moved from a **crate** to the **binary** and found thirteen `eprintln!` sites
# outside `gx-cli`. **1786 -> 1794 over 318**.
#
# 🔴 **A3 / `req/38` §196 (DR-46-9 A-3 / DR-46-10 / DR-46-12)** -- fifteen more, and one of
# them is a new suite, so the suite count moves for the first time in four lanes. Fourteen are
# `crates/gx-adapter-mcp/tests/github16_read_by_tool.rs` (the escrow that reads a prior through a
# declared tool: the ordering against `apply`, two byte-for-byte round trips including a UTF-8 one,
# the JSON pointer resolving one member rather than a document, the `resources/read` road measured
# unchanged, the CAS refusing a third party's write, the two P0 tools that stay `false` by
# mechanism, and the three ways a failed read is answered) and one is a sixth derivation in
# `crates/gx-adapter-mcp/tests/ac_051.rs`, because `read_prior_by_tool` is a **second** road to a
# `tools/call` and AC-051's road count had to be re-derived rather than assumed.
# **1794 -> 1809 over 319**.
#
# 🔴 **R17 / `req/38` §199 (DR-46-15 / DR-46-14)** -- ten more in one new suite,
# `crates/gx-adapter-mcp/tests/r17_attested_object_binding.rs`, which is where the eighteenth
# adversarial audit's H-01 now has a machine: a read declaration whose answer is about an object
# the locator does not name refuses the effect (the arm that measured an undo overwriting a third
# party's write on the previous binary), a read answering with another document's body refuses, a
# declaration-derived failure carries its own sentence and calls nothing, four unsound declaration
# shapes are parse errors, a bound JSON pointer escapes what it substitutes, the refusal honours
# the declared posture, and three controls (a bound deployment still round trips, a third party's
# write still refuses the undo when the faces agree, and the do-and-undo arrival count the LIMITS
# cost line rests on). `github16_read_by_tool` stays at fourteen -- P-9 changed what it asserts,
# not how many. **1809 -> 1819 over 320**.
# 2026-08-18, R19 (req/284, §206): +2 suites `r19_escalation_http` / `r19_escalation_road` (8 probes)
# — the escalation road opened (H-02): ruling verbs and `serve` now read the MCP wiring.
# 2026-08-18, R18 (req/283, §207): +1 suite `r18_declaration_soundness` (8 probes) — declaration
# soundness closed (H-01/M-01/M-03/M-04/L-03/L-04). Combined: **1819 -> 1835 over 323**.
# 2026-08-18, L-01b (req/288, §209): +1 suite `r20_wrap_parse_before_spawn r20_template_prior_soundness r20_undo_that_does_not_restore r20_refusal_vocabulary_is_whole r20_mcp_surface_sentences r22_declaration_gates r22_wrap_road r22_serverless_surface r22_refusal_constant_census r23_cas_declaration_gates r23_wrap_road r23_not_found_road r24_predicate_unification r24_absence_discrimination r24_record_only_and_removal r25_declaration_axes r25_abort_and_record_only dr46_16_cas_read_by_tool r21_refusal_semantics r21_refusal_map_is_whole r21_help_is_user_facing r21_tutorial_fs_walk` (2) + limits_sync lean-recount (1)
# — the numbers a page prints are re-taken from `lean/` on every run. **1835 -> 1838 over 324**.
# 2026-08-19, R26 (req/325, §231): +6 suites from the twenty-fifth adversarial audit's seven
# repairs -- r26_invisible_edge_axis (8), r26_preimage_funnel (6), r26_reach_census (4),
# r26_limits_family_sync (3), r26_not_attempted_causes (9), r26_refusal_remedy_parity (4).
# Plus one probe in the existing `r25_abort_and_record_only` suite: the accepted-residual
# measurement gets an arm per spelling (`req/324` H-01), which is a probe and not a suite.
# **1990 -> 2025 over 351**.
# 🔴 **R29** (`req/38` §238, from the twenty-eighth adversarial audit `req/361`) -- three new
# suites and fifteen new probes: r29_rollback_is_verified (6), r29_instrument_repairs (5),
# r29_rollback_read_faces (4), plus one probe in the existing r26_refusal_remedy_parity suite
# (the walk refusal, driven rather than read). The two numbers move in the same commit as EXPECTED_SUITES below,
# which is req/88 §6.2's rule in hand 7. **2073 -> 2089 over 365**.
# 🔴 **R30** (`req/38` §240, from the twenty-ninth adversarial audit `req/372`) -- four new suites
# and eleven new probes: r30_compensation_precondition (6, the acceptance gate for M-01 -- it drives
# the engine rather than reconstructing its call order, which is why the audit's own arm could not
# see the repair), r30_rollback_window (1 in gx-adapter-fs and 1 in gx-adapter-mcp, publishing the
# residual window as a measured width), r30_journal_backward_compat (1, the producer half of M-02's
# acceptance -- the consumer half runs in a worktree at 3c2cf32 and belongs to that build, so it is
# not counted here), plus one probe each in the existing ac_038 suite (the compensation that is not
# sent for an apply that moved nothing) and floor_doubt (the README's adversarial-audit census).
# The two numbers move in the same commit as EXPECTED_SUITES below. **2089 -> 2100 over 369**,
# and **2100 -> 2108 over 371** with R31 (`req/378` H-02: the two suites that hold the journal
# framing to the marker on its own disk, one at the engine boundary and one end to end),
# and **2108 -> 2128 over 375** with R32 (`req/392` M-01/M-02: the reader's door on a journal of
# zero bytes, and the seven sentences the one refusal paragraph became -- at the two functions, at
# the engine boundary and end to end through the shipped verb),
# and **2128 -> 2142 over 377** with DR-46-24(A) (`req/441`: read_set + fingerprint_scope on the
# wire, the seventh InverseStatus word, and the G4 cost instrument),
# and **2142 -> 2143 over 378** with R33 (`req/397` H-01 / `req/442`: one suite,
# r33_zero_byte_journal_stamp, which pins what a journal file with no bytes in it is answered with
# end to end -- a measurement `req/442` §0-2 (b) asked for and which turned out to be green on the
# unmodified binary, so it is a regression pin rather than an acceptance gate; `req/443` §3 carries
# the paired control run that says so).
# and **2158 -> 2161 over 381** with R34 (`req/449` H-02 / `req/450`: one suite,
# r34_serve_recover_fold, which drives `gx serve`'s start-up fold over all seven declared
# `RecoveryPath` roads plus the two terminal states an `ApplyWasAnnounced` row can end in --
# eight rows -- asserts the two sentences 43 §7-3c's road owes an operator, and pins the four
# members of RECOVERY_REBUILD_DISAGREES (req/449 M-01). The audit could
# only measure that fold by copying it into its own probe, which is a second source of truth;
# this suite drives the shipped one.
# and **2162 -> 2178 over 383** with S1 (`req/452` / DR-46-26, `req/38` §258: two suites,
# dr4626_invert_seam -- the engine end of the widened `SubstrateAdapter::invert`, driving the
# read-set to the signed bytes and `InverseStatus::Undetermined` to an escrow row, each with its
# negative control and with the one-`invert`-call-per-stage count that says the seam costs no extra
# read -- and inverse_status_wire, which asserts the fourteenth 42 §3.10 field as a **subtraction**
# from D24's golden (remove one key, get D24's bytes back) rather than as a regenerated constant.
# and **2178 -> 2182 over 385** with DR-46-27 (`req/454` / E-DR4627-1, `req/38` §261 ruling 5: two
# suites for one field. decided_at_seat holds the killer assumption in three pieces -- a source scan
# that Cedar's seven-tuple projection still does not read `decided_at` (PS-1, with a presence arm so
# the absence arm cannot pass vacuously), a differential sweep of five `decided_at` values spanning
# `i64` across all three `Verdict` arms that compare equal, and a registered stub invariant proving
# those five values actually arrived -- which is what makes the invariance a fact about the verdict
# rather than about a field that never left the struct. decided_at_wire is the other end: the engine
# is asked at a moment one hour after it planned, and the invariant sees the *verify* moment, not
# `t.created_at`. Neither suite is a window predicate; DR-46-27 ships the seat and no predicate, and
# the differential one is expected to go red on purpose the day the first real `now ∈ [a,b]`
# invariant is written.
# and **2178 -> 2195 over 385** with D28 (`req/459` / DR-46-28, `req/38` §255 ruling 4: two
# suites, boundary_attest -- the KA battery, one bed per value of the taxonomy, each built to make
# that value false and each asserted against the production refusal rather than against a restated
# claim, plus the scan that says the attest never reaches the derivation it attests -- and
# dr46_28_boundary_declaration, the catalogue's fourth reserved slot, where a misspelt declaration
# is a parse error and the declaration meets the attest in one join.
# Merged floors (req/38 SS266): d27 added 4 probes / 2 suites and d28 added 17 probes / 2 suites
# on the same 2178/383 base, so the tree that carries both carries their sum.
# and **2178 -> 2182 over 384** with R35's red bed (`req/470` H-01 / `req/471`: one suite,
# r35_shared_road_sentence, which drives the four shipped verbs audit 34 measured walking 43
# §7-3c's road in silence -- `gx verify`, `gx commit`, `gx undo`, `gx repair --yes` -- plus the
# two roads that must stay silent, plus `gx wrap`, whose membrane three consecutive audits
# declared un-driven and reasoned about from source instead. Committed **red** on purpose
# (`req/38` §226): the assertions fail at the commit that adds them, which is what says the bed
# can fail at all.
# and **2182 -> 2186 over 385** with R35's repairs (`req/470` M-02/L-01/L-02: one suite,
# gx-adapter-fs's pack_locators -- PACK_FORMAT F7's fourth instrument, the one the fs pack has
# never had, driving the shipped `policies/fs/scenarios.json` through the adapter's own
# `normalize`/`is_absolute` rather than restating the grammar -- with its own negative control;
# plus two in gx-gate's pack_v0: F3's mechanism over every shipped pack with a fifth pack that
# declares neither default as the negative bed, and F6's read count taken from a real
# `RequestView` against the row that said seven.
# Merged floors (req/38 SS268): r35 added 8 probes / 2 suites on the 2178/383 base,
# on top of the d27+d28 union 2199/387, so the tree that carries all three carries 2207/389.
# and **2199 -> 2203 over 388** with DR-46-31 (`req/473` / `req/38` §261 ruling 2b: one suite,
# dr4631_escalated_reissue -- the bed for the re-issue of a commit a person allowed, plus three
# controls: a pre-DR-46-31 journal still refuses (so replay is reading the digest and not deriving
# it), a forged digest still refuses (so the leaf comparison is live), and the same rewrite road
# with the witnessed digest files (so the two refusals are about the digest and not about the
# road that produced them). dr4626_invert_seam does not change count: its blocker assert became
# the arm it was blocking.
# Merged floors (req/38 SS270): d31 added 4 probes / 1 suite on the 2199/387 base,
# on top of the r35 union 2207/389, so the tree that carries all four carries 2211/390.
# R36 (req/38 SS271 ruling 5, report req/480): +13 probes / +3 suites on the 2211/390 base.
# r36_error_road (7) is req/476 H-01's bed -- the four write verbs plus `gx serve` on the `Err`
# road, a second of the eight steps after `apply_once` (record_head, not only file_receipt), the
# Err-arm census, and audit 35's two negative controls rebuilt. r36_f3_edges (2) is the
# seven-shape table for F3 plus the shipped four. r36_catalogue_duplicate_key (3) drives what
# req/476 s3-2 had only read. decided_at_seat gains one (ps_1b, the mutant audit 35 s7-4 declared
# it had not built), which is a probe in an existing suite and moves the probe count only.
# R37 (req/501, report req/502): +11 probes / +4 suites on the 2224/393 base.
# r37_error_road_telling (4) is req/496 M-01 + M-02's bed -- the record_head road driven at all
# three `Engine::recover` call sites (repair.rs, session.rs through `gx verify`, serve.rs) with the
# journal counted on each, plus audit 36's no-earlier-commit control re-run and declared
# non-discriminating for the reason R7's laundering guard gives. r37_ledger_gate_and_state_shape (2)
# is M-04's three ledger reads before and after a cut plus L-02's two mouths on one row.
# r37_f3_and_reads (3) is M-03's five-shape table, its two refusal controls, and L-01's census that
# the convention guard exists and `ps_1` calls it. r37_recover_partial (1) is M-02's discriminating
# control, at the engine because the two-`Committing`-row project a CLI bed would need is not
# constructible by truncation (the suite's own header carries the argument). decided_at_seat gains
# one (ps_1c, the three spellings audit 36 wrote, each one seen or refused), which is a probe in an
# existing suite and moves the probe count only.
# R38 (reqdef req/517, report req/519): +4 probes / +2 suites on the 2254/401 base.
# r38_ledger_face_width (1) is req/513 M-01's family driven on the CLI face -- the three `gx log`
# verbs and `gx receipt verify`'s local-ledger anchor, before and after a journal cut, with the
# argument-question controls that hold the gate below the caller's argument and a second project
# that was never cut. frozen_receipt_corpus (3) is req/38 s294-2 (b): req/280's 2026-08-18 receipt
# frozen byte for byte, asserted separately to carry a good signature and to decode. decided_at_seat
# gains no probe (ps_1c gains two cases inside the existing one) and pack_v0 gains none (the
# non-comment case is inside `every_shipped_pack_declares_its_default`), so both move nothing.
# 🔴 S④ SDK-verify hardening (`req/503`, report `req/509`): **+6 probes / +0 suites** on the
# 2240/400 union base, and **+5 on `MIN_SDK_PASS`** below. The split across two numbers is R12's
# rule and it does real work here: this lane's repair spans a Rust crate and a `node --test` suite,
# and folding them would hide which side moved.
#
# All six cargo probes land in `sdk/wasm-verify`'s existing `#[cfg(test)] mod tests` -- a
# lib-unittest line `MIN_SUITES` already counts, so **no suite arrives** and 400 does not move.
# The crate's own line reads `5 -> 11`. The six:
#   1. `a_genuine_receipt_and_a_genuine_signed_head_verify`  -- the authenticated control
#   2. `a_head_whose_signature_was_flipped_is_refused`       -- E-SDK-10 negative 1
#   3. `a_head_whose_origin_was_changed_is_refused`          -- E-SDK-10 negative 2
#   4. `a_head_naming_another_tree_size_is_not_a_pass`       -- the **discriminator**: already green
#      in Rust before the repair, and that is the finding. It separates E-SDK-9 (a stale *binary*)
#      from E-SDK-10 (a missing *check*); if it ever goes red here the fault is in the engine.
#   5. `a_checkpoint_key_with_no_checkpoint_is_refused`      -- the usage arm gx-cli already had
#   6. `anchor_authenticated_is_present_on_every_answer`     -- M6H8-11 adopted (a), ported
#
# The +5 on `MIN_SDK_PASS`: `test/wasm_vocab_freshness.test.mjs` (3 -- E-SDK-9's permanent stale
# detector, which reads the vocabulary out of the Rust rather than copying it) and two additions to
# the existing `test/audit_m9_p4_tamper_and_errors.test.mjs` (E-SDK-8: a non-string argument is
# refused rather than faulting WASM memory, and the absent forms still mean absent).
# req/521 gx-adapter-mysql (report req/526): **+57 probes / +4 suites** on the 2254/401 base.
# 40 unit tests in `crates/gx-adapter-mysql/src/` (locator 6, sql 15, db 6, delta 5, row 4,
# +4 shared), 16 in `tests/mysql_offline.rs`, 1 in `tests/mysql_conformance.rs`. The suite delta is
# four and not three because the crate's doc-test target counts as a suite alongside its lib target
# and its two integration targets -- measured from f1's own reconstruction (2311/405), not derived
# by hand. `mysql_conformance` needs a live server: without `GX_ADAPTER_MYSQL_DSN_DEFAULT` the
# cargo test target goes RED (measured at the 2026-08-22 merge turn — the NOT_RUN verdict exists
# one layer down, in gx-substrate-conformance, not at this level), joining the same known-red
# family as the 19 postgres tests. `mysql_offline` needs nothing and is the reason this adapter
# still reports a number on a machine with no MariaDB.
# 🔴 P-1a attach placement (`req/535` §3 R-1 / §4 AC-1, AC-2', AC-3): **+8 probes / +1 suite** on
# the 2256/401 base. One new suite file, `crates/gx-cli/tests/p1a_attach_placement.rs`, because the
# subject is a verb that did not exist and there was no suite to add it to. The eight:
#   1. `ac1_attach_enumerates_every_declared_path_in_three_classes` -- AC-1, all eleven GX_PATHS
#      rows classified, with the denominator read out of the library rather than transcribed
#   2. `ac1_control_an_enumeration_missing_one_row_is_refused`      -- AC-1's negative column, run
#      eleven times (one per row removed from the real answer) plus an invented class word
#   3. `ac2_attach_moves_no_tracked_file_and_names_what_it_added`   -- AC-2', both halves
#   4. `ac2_control_a_touched_tree_and_a_short_declaration_are_refused` -- AC-2's negative column,
#      and the arm that **caught this suite's own first instrument**: `git ls-files -s` prints the
#      index, so a rewritten working-tree file came back identical and the positive arm proved
#      nothing. The digests are `git hash-object` of the files on disk now
#   5. `ac3_attach_completes_with_no_network_and_the_namespace_is_proven_empty` -- AC-3 under
#      `unshare -rn`, with the namespace's own emptiness measured first
#   6. `r1d_a_project_outside_the_tree_leaves_the_tree_with_nothing_added` -- R-1d
#   7. `attach_twice_creates_nothing_the_second_time`               -- placement is idempotent
#   8. `the_existing_verbs_are_untouched_and_attach_is_the_new_one` -- the twenty-two verbs
#      `req/535` §1 counted are still listed, read off clap rather than transcribed
#
# (merge-turn correction, req/38 §311: the "+1 that is not this lane's" this comment once alleged
# was not hollowing — the lane's base e89836ba sat between f10's arrival and its ratchet commit
# 0ca010a0, one commit later. No probe ever shipped un-ratcheted on main.)
# R39 (reqdef req/540, report req/542): +15 probes / +1 suite on the 2318/407 base.
# r38_ledger_face_width gains 6 (audit 38's b1-b4 and c0-c3 beds, moved inside the tree, plus the
# escape-one control and req/533 L-03(b)'s but-for clause and L-05's two-branch withdrawal record),
# frozen_receipt_corpus gains 3 (the declared set over the bytes, the same set over the schema, and
# the set against the page), r37_f3_and_reads gains 1 (req/533 L-01's three residues with two
# controls), and r39_frozen_receipt_verdict is new with 5 (docs/LIMITS.md said `gx receipt verify`
# answers exit 7 for the frozen document and nothing ran the binary against it, because the corpus
# suite lives in a crate that has no binary to run).
# P-1b (reqdef req/544, report req/548): +15 probes / +4 suites on the 2348/410 base, measured by
# `floor_doubt` f1 rather than added up by hand (req/38 §307-3). `p1b_attach_face_frozen` is 3 (the
# two frozen attach-face specimens still verify offline, the census that refuses a re-mint, and the
# provenance read off the frozen attach answer), `p1b_coverage_declaration` is 5 (AC-14's verbatim
# survivors, AC-5 (a)'s refusal of a declared measurement, AC-9's unmet rows, AC-11's two levels and
# F-3's invariance of the receipt table under two different faces), `p1b_coverage_totality` is 4
# (AC-12's walk of every receipt document in the tree, its hand-picked-denominator control, every
# spelling of every column, and F-1's source census) and `p1b_coverage_wire` is 3 (AC-4's pairwise
# distinctness, the omission spelling's collapse, and AC-5 (b)'s promotion census).
# P-1c (reqdef req/551, report req/555): +18 probes / +1 suite on the 2363/414 base, measured by
# `floor_doubt` f1 rather than added up by hand (req/38 §307-3). `p1c_detach` is 18: AC-6's two
# readings of idempotence (state, and the exit that is 1 rather than the 2 the reqdef predicted),
# AC-7's byte-identical check either side of the round trip, AC-8's frozen specimens verifying after
# a detach plus the no-checkpoint control that proves the verification is one a deletion breaks,
# AC-15's two "nothing to undo" answers, AC-16's named non-restorations with the omission control,
# AC-17's untouched neighbours, AC-18's three words, AC-19's empty namespace and unchanged tree with
# their two controls, AC-20's surviving sentence, the two falsifiers that fired when measured
# (G-1's flag whose value is the separator, G-2's preserved key order) and four refusal shapes.
# S③ AC-6 (req/493 §1 AC-6, report req/557): +3 suites on the base above -- the count is
# `floor_doubt` f1's, not this comment's (req/38 §307-3).
# confinement_attest (gx-witness) is the schema half: the two pairs that are not states of the
# world each get a bed and a refusal, all four (enforced x confined) pairs are asserted legal so
# "orthogonal" is a measurement rather than a word, and the wire claim is a subtraction against a
# shadow declaration of the pre-erratum shape rather than a stored golden.
# confinement_receipt (gx-engine) is the producer half, and its third probe is the load-bearing
# one: a confined commit is re-issued by an engine declaring a DIFFERENT confinement, and the
# rebuild has to answer Filed with the FIRST value -- which is what says the value came out of Sigma
# and not out of the running process. Without it, an implementation that re-read the environment at
# rebuild time would pass every other probe here and answer payload_mismatch (the word for
# tampering) on every crash recovery of a commit made inside a `gx confine`.
# confine_receipt (gx-cli) is the road through the binary: the kernel->environment hop measured by
# running a real `gx confine` and reading the variable out of the process it becomes (Linux only),
# the grammar with ten refused beds, and the receipt read out of `.gx/receipts/` as a stranger
# would read it -- out of the signed payload, not out of the JSON around it.
# leaf-from-signed-bytes (req/38 §324 ruling 3, the precondition the above rides on):
# canonical_bytes_road (gx-canon) and leaf_from_signed_bytes (gx-witness) -- 2 more suites.
# R40 (req/553 M-01 + L-02, ruling req/38 §328, report req/558): +2 suites on the base above --
# r40_journal_presence (8: the three unreadable forms refuse with no signature, the third-party
# verifier still reads, one condition wears one word on both roads) and r40_serving_routes (2:
# /healthz and /ledger/* answer 500 while GET /receipts/{tid} stays 200, body byte-identical).
# The union count below is floor_doubt f1's measurement, not arithmetic over the two branches.
# R41 (req/561 + its S11 supplement, ruling req/38 S333, report req/564): +24 probes / +1 suite on
# the base above -- r41_presence_unification (24: the last five is_file()/exists() folds on the
# read doors that skip Layout::open -- keys.rs load/load_encrypted, ledger::open, replay::open,
# verdict::list_from_file -- each driven through absence, directory-shape and unstatable-parent
# arms; 10 ka1_* probes pin the downstream doors' Err contract the pass-through rests on; and
# audit 40 F-1's bed-E/bed-D on gx repair --json, where ledger_present/verdict_chain_present now
# answer null rather than false about files this process could not stat).
# frz1 (merge 9ae910b4, req/573, verified req/583, ratchet fix req/38 §353-2): +4 probes / +1 suite
# on the 2439/423 base, measured by `floor_doubt` f1 rather than added up by hand. The merge landed
# `crates/gx-witness/tests/frozen_receipt_corpus_gx_p5_tutorial.rs` (the gx_p5_tutorial frozen
# receipt corpus + tfcore_live pubkey export) without raising the declaration alongside it, so the
# clone gate sat RED at the base until this hand -- req/29 §4's "adding a suite is two places at
# once" violated once and repaired here, per §353-2's ruling that a merge which lands a suite file
# owes the ratchet in the same commit or the next one that touches this file.
# R43 (req/578, ruling req/38 §350, report req/591): +4 probes / +1 suite --
# r43_presence_and_head (4: the `verdict_chain_present` fold on `repair_and_report`'s **healthy**
# road, which R41's S-6 scope sentence left standing one function away; `head_recorded` split from
# the value `witnessed` is computed from, so a head that will not read answers null and the exit
# does not move; `declared_directories_are_directories` refusing a declared row it could not stat
# instead of passing it silently; and one probe that measures that `repair`'s per-directory row is
# not reachable under either unstatable shape, which is why that third `is_dir()` coordinate is
# registered rather than converted).
#   ⚠ +4 probes / +1 suite of the raise below are **not** this lane's:
#       `frozen_receipt_corpus_gx_p5_tutorial` (commit 22d603b0, req/568 §4, ruling req/38 §338)
#       landed without being named in EXPECTED_SUITES or counted in MIN_PROBES, so f1/f2 were
#       already red at this lane's base (measured on 4cba77f8 with this lane's own suite moved
#       aside: FLOOR_DECLARED=2439/423 FLOOR_RECONSTRUCTED=2443/424). MIN_PROBES is one equality
#       and cannot be raised for one suite only, so the number below closes both -- the same shape
#       as `journal_changelog_doubt` above, and reported the same way (req/591).
# Union measured by floor_doubt f1's own FLOOR_RECONSTRUCTED, not arithmetic: 2447/425 on the r43
# tree (which lacked r42's +2). Merge union of d2a0eb5a (r42, +2 probes) and r43 (+4/+1): the
# numbers below are that union's expectation, and the f1 confirm run recorded in req/606 is the
# measurement of record for this declaration (req/38 §355: declarations move on f1, not arithmetic).
# rmcp1 (req/602, verified req/617): +15 probes / +1 suite (rmcp1_github_p1) on its own tree
# (measured 2460/425 there, which lacked r43/p2ii). Merge union with b6cd759b (2483/432): the
# numbers below are that union's expectation; the f1 confirm run recorded in req/619 is the
# measurement of record (req/38 §355: declarations move on f1, not arithmetic).
# R45 + R44 lane B union (req/38 §407): merge of r45a (repair.rs M-1/L-2/L-3, +7 probes / +1 suite
# `r45a_repair_lane`, verified req/650 twice-blind), r45b (r41/r43 negative controls + L-4, +2 probes,
# req/651), and r44b (attach unreadable_entries + spec §1.2, +2 probes, req/652 -- fmt drift fixed at
# 0b6c4904 before merge). Union measured by floor_doubt f1's own FLOOR_RECONSTRUCTED on the merged
# tree 44971a9f, not arithmetic: **2509/434** (#[test]=2499 doc=10 | integration=393 libs=18 bins=5).
# The one new suite `r45a_repair_lane` is added to EXPECTED_SUITES below in the same commit (req/88
# §6.2: raise MIN_PROBES/MIN_SUITES and name the suite at once). MIN_SDK_PASS unchanged (27): no lane
# touched the SDK surface.
# R45-c (req/38 §408): merge of r45c (rmcp1_github_p1 M-4 positive-control + L-6 discriminating
# asserts, +2 probes on an existing suite, verified req/655 -- twice-blind with req/653). No new
# suite (rmcp1_github_p1 already in EXPECTED_SUITES), so MIN_SUITES holds at 434. Union measured by
# floor_doubt f1 on merged tree 8336eafa, not arithmetic: **2511/434** (#[test]=2501 doc=10).
# 654 + DR-46-38 batched (req/38 §411): merge of fix_ledger_present (ledger_present road unification,
# +4 probes / +2 new suites ledger_present_road_parity + ledger_present_road_doubt, verified req/656)
# and dr46_38 (CasArgSource resource_suffix_number, +4 probes / +0 suite inline, verified req/660).
# 🔴 recovery: commit ce1cf7a3 had silently reverted the R45-c ratchet (rmcp1_github_p1 17->15,
# MIN_PROBES 2511->2509, README/public floor + genealogy) via a contaminated working tree; §411
# restored rmcp1 from 3fac6f57 and the floor files from d1680410, then ratcheted. Union measured by
# floor_doubt f1 on the corrected tree, not arithmetic: **2519/436** (#[test]=2509 doc=10).
# R44-A (req/38 §413/§414): item8a header census gate -- new suite header_census_doubt
# (0-missing SPDX+Copyright invariant predicate over crates/+probes/, +2 probes / +1 suite,
# verified req/668). Measured by floor_doubt f1 on the merged tree, not arithmetic: **2521/437**.
# dr4633p1b (req/38 DR-46-33 path1, req/669): input-generation declaration (a-prime) -- new
# integration suite dr4633_input_generation (optional InputStageDeclaration trait + journalled
# join), +4 probes / +1 suite. Measured by floor_doubt f1 on the merged tree, not arithmetic: **2525/438**.
# P1 receipt-conformance (req/506 P1, merge of 1b866d03 p1_receipt_conformance): the permanent
# receipt-format conformance suite for LIMITS #8's primary claim -- a third party verifies a receipt
# with three files and one binary -- in A-90's compatible/verified/refuted vocabulary with a
# negative-control catalogue (+6 probes / +1 suite `receipt_conformance`, source untouched, tests/ only;
# red-first confirmed: a verifier collapsing the three verdicts fails 5/6). Measured by floor_doubt f1's
# own FLOOR_RECONSTRUCTED on the merged tree, not arithmetic: **2538/440**
# (#[test]=2528 doc-tests=10 | integration=399 libs=18 bins=5).
# Phase B follow-up (merge of c6b4d335 phaseb_followup, AC-B4/B5/B7 + `gx checkpoint export
# --note-out` / `gx checkpoint audit` verbs): +3 suites `phase_b_witness_cli` / `witness_offline` /
# `equivocation_refusal_doubt` (all named in EXPECTED_SUITES below), +11 probes; source changes in
# crates/gx-cli/src/{ledger.rs,main.rs} only. Union measured by floor_doubt f1's own
# FLOOR_RECONSTRUCTED on the merged tree, not arithmetic: **2549/443**
# (#[test]=2539 doc-tests=10 | integration=402 libs=18 bins=5).
# P2 CRUD-22 capability conformance (req/506 §1 P2, cherry-pick of 61df37b5 + 461ead76
# p2_crud22): the permanent resource×CRUD matrix conformance suite -- every occupied cell
# answers over the shipped router, every structurally-empty cell has no route tied to one of
# eleven reason constants, data-driven over the crate's own SPECIFIED/EXTENSION/
# VERDICT_CHECKPOINT endpoint lists; red-first negative control (an injected DELETE
# /transformations/{id} turns the emptiness check red). +4 probes / +1 suite
# `crud22_conformance` (source untouched, tests/ only; no frozen-face or Lean contact).
# Union measured by floor_doubt f1's own FLOOR_RECONSTRUCTED on the merged tree, not
# arithmetic: **2553/444** (#[test]=2543 doc-tests=10 | integration=403 libs=18 bins=5).
# hash_injectivity (req/38 SS554/SS630, crates/gx-canon/tests/hash_injectivity.rs): the test half
# of the gap SS554 named -- gx-canon claims hash injectivity/prefix-freeness in prose but had
# neither theorem nor test. +5 probes / +1 suite `hash_injectivity` (named in EXPECTED_SUITES
# below), tests/ only, no source change. The suite file landed with wave A but was never
# ratcheted into this declaration (SS279 duty missed; f1/f2 caught it at SS630). Measured by
# floor_doubt f1's own FLOOR_RECONSTRUCTED on the tree that already held the file, not
# arithmetic: **2558/445** (#[test]=2548 doc-tests=10 | integration=404 libs=18 bins=5).
# DR-46-21 merge (req/680, LANE GATE-EXEC item 2, SS631 item 3 executed under Owner #326-#329 full
# delegation): +4 probes / +1 suite `dr46_21_digest_reverify` (named above), tests/ only, no other
# source-count change (cas.rs/invert.rs/lib.rs/wrap.rs additions are non-test code). Measured by
# floor_doubt f1's own FLOOR_RECONSTRUCTED on the merged tree, not arithmetic:
# **2562/446** (#[test]=2552 doc-tests=10 | integration=405 libs=18 bins=5).
# Cargo-queue omnibus (req/773 SS2 / req/775, 2026-08-25): +1 suite landed by a concurrent lane
# (`b_audit_m1_fork_classification_reachable`) + 3 suites this omnibus landed (DR-46-25 cost probe
# `dr4625_read_set_cost_probe`, DR-46-39 attest `dr4639_catalogue_hash_attest`, req/508 phase 3
# gate `req_graph_doubt`, named in EXPECTED_SUITES below). Measured by floor_doubt f1's own
# FLOOR_RECONSTRUCTED on the merged tree, not arithmetic: **2587/450**
# (#[test]=2577 doc-tests=10 | integration=409 libs=18 bins=5).
# DR-46-40 impl (req/730, `crates/gx-witness/tests/dr4640_standing_windows.rs`, named in
# EXPECTED_SUITES below): +8 probes / +1 suite, tests/ only plus the additive StandingEntry/
# StandingLedger/Standing type in keys.rs (non-test code, does not move probe/suite counts).
# Measured by floor_doubt f1's own FLOOR_RECONSTRUCTED on the merged tree, not arithmetic:
# **2595/451** (#[test]=2585 doc-tests=10 | integration=410 libs=18 bins=5).
# req/529 residual-5 cells (req/773 SS2 atom 6, req/775): 3 new suites, live fault injection --
# `dr529_residual_cells` in gx-log (log x missing-field, log x order-swap), `dr529_missing_field`
# in gx-canon (canon x missing-field), `dr529_cli_order_swap` in gx-cli (cli x order-swap), all
# named in EXPECTED_SUITES below. Measured by floor_doubt f1's own FLOOR_RECONSTRUCTED on the
# merged tree, not arithmetic: **2601/454** (#[test]=2591 doc-tests=10 | integration=413 libs=18
# bins=5).
# req/801 cargo lane (G-07/G-08 + GH013 + tamper exit): +1 probe, 0 new suites -- the `--json`
# tamper-exit pin `the_json_flag_does_not_soften_the_tamper_exit` in the existing
# `verdict_checkpoint_surface` suite (gx-cli), closing req/792 §2b's exit-0 measurement question
# on the artefact. Measured by floor_doubt f1's own FLOOR_RECONSTRUCTED, not arithmetic:
# **2602/454** (#[test]=2592 doc-tests=10 | integration=413 libs=18 bins=5).
MIN_PROBES=2602
MIN_SUITES=454
# s1 generated_at freshness (req/38 §287-3, probes/doubt/tests/semantics_doubt.rs): +1 probe
# (days_since_epoch_matches_known_civil_dates, a self-test for the date arithmetic
# check_generated_at_is_fresh depends on) on the 2253/401 base. No new suite file.
# D-34 (req/494 + req/498, report req/510): +7 probes / +1 suite on the 2240/400 base.
# dr4634_read_set_absence (7) is DR-46-34's bed: the four preimages of `read_set: null` -- an
# escrow that read nothing, a rebuild with no `InverseEscrowed` record, a rebuild over a journal
# that predates 42 SS3.13's `reads`, and a `VerdictReceipt` -- driven on the roads that make them,
# plus the pairwise-distinct byte comparison and the one-bit control that decides the third
# (`reads_attested` true files the re-issue, false refuses it, and nothing else differs), and
# two backward-compatibility controls: a record with the flag `false` puts no key on the wire
# (so every journal ever written re-encodes identically) and a CommitReceipt carrying the
# pre-DR-46-34 `null` still schema-checks and still round-trips through a signature.
# The SDK gains inverse_status_vocabulary_parity (2 node tests, counted on MIN_SDK_PASS): the
# engine's seven `InverseStatus` words against the SDK union's, as an equality -- the gate that
# was missing when the union carried six.
# 🔴 **R12 / `req/242` M-04** -- the SDK's own suite, counted on its own line.
#
# The audit measured `sdk/typescript` sitting outside this floor entirely: `tools/e2e.sh` ran no
# `npm` and no `node`, and the SDK's `gx_code` census imported `../dist/index.js` -- a build output
# `.gitignore` excludes -- so it compared the server's source against an **artifact**. Two things
# changed: the census reads `src/errors.ts`, and this script runs the suite (stage 5b below).
#
# A separate number rather than a bigger `MIN_PROBES`, declared in one line: **`MIN_PROBES` counts
# lines cargo printed, and folding a different runner's total into it would make one number mean
# two things.** `node --test`'s own `# pass` / `# fail` / `# skipped` are read instead.
# R44-E / L-02 (`req/673` §3(a), `sdk_health` branch merge): +3 node tests in
# `server_health_vocabulary_parity.test.mjs` -- the `Receipt.server_health` object (status
# vocabulary + object keys) checked as an equality against `crates/gx-api/src/handlers.rs`, plus
# the `Receipt` interface's own optional-field exposure. 27 -> 30.
MIN_SDK_PASS=30
MAX_SDK_SKIP=7

# P5 (req/134 §1 items 1/7, `crates/gx-cli/tests/demo_e2e.rs` + `crates/gx-cli/tests/
# limits_sync.rs`): +3 `#[test]` functions (1 in demo_e2e.rs, 2 in limits_sync.rs) and +2
# suites (one integration-test binary per file) over req/38 §79's 1473/276 baseline.

# The floor above counts; it does not ask WHICH suites ran. Deleting the four semantics
# probes and putting four empty tests in their place still prints `GREEN 36 probes over
# 5 suites` (req/08 V§4 N-1, run V4): the same disease as B-4 at a lower dose. So the
# names are declared here as well, and the set that ran must equal this set exactly --
# a suite that vanished and a suite that was swapped are the same lie. A mismatch is RED,
# never a warning: a warning absorbs the failure and leaves the run green (req/16).
EXPECTED_SUITES='ac_001 ac_002 ac_003 ac_004 ac_005 ac_006 ac_007 ac_008 ac_009 ac_010 ac_011 ac_012 ac_013
ac_014 ac_015 ac_016 ac_017 ac_018 ac_018_cli ac_019 ac_019_cli ac_020 ac_020_cli ac_021 ac_022
ac_023 ac_024 ac_025 ac_026 ac_028 ac_029 ac_030 ac_030_cli ac_031 ac_032 ac_033 ac_034 ac_035
ac_036 ac_037 ac_038 ac_039 ac_040 ac_041 ac_042 ac_043 ac_044 ac_045 ac_046 ac_047 ac_048
ac_049 ac_050 ac_051 ac_052 ac_053 ac_054 ac_055 ac_056 ac_057 ac_058 ac_059 ac_060 ac_069
ac_070 ac_071 ac_071_072_cli ac_072 ac_073 ac_073_cli ac_074 ac_p1_1_escrow_apply_offline_verify
ac_p1_2_undo_round_trip ac_p1_3_scope_out ac_p1_5_concurrent_write_cas ac_p2_3_key_encryption
ac_vc adapter_contract adapter_spec append_idempotence apply_durability audit_m9_p1_db_attack
audit_m9_p1_locator_attack audit_m9_p1_sql_attack audit_m9_p2_key_tamper
audit_p3_a1_b2_agent_bypass audit_p3_a2_fail_posture audit_p3_a3_concurrency
audit_p3_a4_crash_retry audit_p3_a6_record_only audit_path_length audit_v04l_api_repairs
audit_v04l_engine_repairs auth authority_boundary b_audit_m1_fork_classification_reachable
base64_vectors bench_gate_doubt binary_e2e
blob_store boundary_attest broken_fixture catch_up_eviction catch_up_scan checkpoint_core
canonical_bytes_road checkpoint_signature
cid_text cjk_doubt confine_receipt confinement_attest confinement_receipt
leaf_from_signed_bytes commit_protocol compose compose_range concurrent_commit config_adoption
conformance conformance_gen contracts_seven core_error_vocabulary crash_recovery crud22_conformance
d24_read_set_cost decided_at_seat decided_at_wire declaration_writer_doubt declared_limits
defaults delta_semantics
delta_skeleton demo_e2e deny_order do_result dr2 dr4625_read_set_cost_probe dr44_9_views dr4626_invert_seam
dr4631_escalated_reissue dr4633_input_generation dr4634_read_set_absence dr4639_catalogue_hash_attest
dr4640_standing_windows
dr46_16_cas_read_by_tool dr46_21_digest_reverify dr46_28_boundary_declaration dr529_missing_field
dr529_residual_cells dr529_cli_order_swap dr_v4b_2_const_json dr_v4b_3_strip dr_v4b_3_wrap_strip draft_index
endpoints enforce enforcement_axes engine_shape error_vocabulary escalation escrow_types evidence_cid
exit_map exit_matrix_cli false_admit fault_injection fingerprint_identity floor_doubt fold_doubt
forward_ceiling fs_delta gate_conformance_gen gate_input_spec gate_shape git_commutation
git_conformance git_delta git_plan_purity github16_read_by_tool github_target_catalogue
golden_vectors gx_layout h2_normalised_before_the_gate harness_shape hash_injectivity id_resolution
idempotency
identity_id identity_view incremental_inclusion intent_identity invariant_registry
inverse_status_wire invert_ceiling journal_changelog_doubt journal_identity journal_roundtrip
journal_vocabulary key_lifecycle_cli key_surface laws ledger_doubt ledger_durability
lifecycle_states lifecycle_transitions limits_sync lists locator_normalisation log_commands
log_error_vocabulary m2_types m3_types m4_types m6_exit_matrix m6_gx_code m6_stream_map
m6_surface_doubt m6h5_cli m6h6_cli m6h7_api m6h7_delivery map_key_order mcp_commutation
mcp_conformance mcp_delta mcp_plan_purity mcp_restore_template method_classification mint_domain
model_a_probes negative_vectors nfr_027 notion_page_catalogue opacity otel_export
mysql_conformance mysql_offline fallible_step_doubt
overclaim_doubt p1a_attach_placement p1b_attach_face_frozen p1b_coverage_declaration
p1b_coverage_totality p1b_coverage_wire p1c_detach p2_auth_doubt pack_embedding pack_locators pack_v0
pae_golden pg_conformance
phase_b_witness phase_b_witness_cli witness_offline equivocation_refusal_doubt pipeline_cmds plan_purity planned_delta_identity policy_cmds policy_determinism policy_mapping
postgres_db_e2e postgres_wired print_consumers proof_digest r17_attested_object_binding
r18_declaration_soundness r19_escalation_http r19_escalation_road r20_mcp_surface_sentences
r20_refusal_vocabulary_is_whole r20_template_prior_soundness r20_undo_that_does_not_restore
r20_wrap_parse_before_spawn r21_help_is_user_facing r21_refusal_map_is_whole
r21_refusal_semantics r21_tutorial_fs_walk r22_declaration_gates r22_refusal_constant_census
r22_serverless_surface r22_wrap_road r23_cas_declaration_gates r23_not_found_road r23_wrap_road
r24_absence_discrimination r24_predicate_unification r24_record_only_and_removal
r25_abort_and_record_only r25_declaration_axes r26_invisible_edge_axis r26_limits_family_sync
r26_not_attempted_causes r26_preimage_funnel r26_reach_census r26_refusal_remedy_parity
r27_census_derivation r27_edge_class_width r27_limits_probe_counts r27_parity_allowlist
r27_reentrant_abort r28_abort_answer_sweep r28_cell_count_claims r28_completion_facts
r28_probe_counter_discrimination r28_remedy_marker r28_rollback_members r29_instrument_repairs
r29_rollback_is_verified r29_rollback_read_faces r30_compensation_precondition
r30_journal_backward_compat r30_rollback_window r31_e2e_empty_journal_submit
rmcp1_github_p1
r31_journal_format_from_disk r32_conditional_diagnosis r32_note_is_conditional
r32_readers_door_zero_byte r32_zero_byte_report r33_zero_byte_journal_stamp
r34_serve_recover_fold r35_shared_road_sentence r36_catalogue_duplicate_key r36_error_road
r36_f3_edges r37_error_road_telling r37_f3_and_reads r37_ledger_gate_and_state_shape
r37_recover_partial r38_ledger_face_width frozen_receipt_corpus frozen_receipt_corpus_gx_p5_tutorial
r39_frozen_receipt_verdict r40_journal_presence r40_serving_routes r41_presence_unification
r43_presence_and_head r45a_repair_lane ledger_present_road_parity ledger_present_road_doubt header_census_doubt frozen_receipt_corpus_gx_p5_tutorial
raw_jsonrpc read_set_wire receipt_conformance receipt_disclosure receipt_kind_branch req_graph_doubt
receipt_verdict_wire receipt_verify_hermetic receipt_verify_history record_only_e2e
record_only_per_call replay_cmd residual revocation router rule_two scope scope_bound
scope_elision secret_scan semantics_doubt serve_runtime_e2e serve_runtime_r1b serve_runtime_r2
serve_runtime_r3 serve_runtime_r4 serve_runtime_r5 serve_runtime_r6 serve_runtime_r7 shipped_set
shutdown sigma_replay state_machine_coverage store_shape stream subject_index substrate_contract
substrate_error supersede term_doubt ticket_rehydration tile_wire two_phase_escrow undo_cas_e2e
undo_cas_mcp undo_cmd undo_settle unsafe_forbidden value_range_closure verdict_checkpoint_store
verdict_checkpoint_surface verdict_checkpoints verdict_identity verdict_meet verdict_order
wire_census wire_handshake witness_conformance_gen witness_error_vocabulary workspace_doubt
writer_doubt'
EXPECTED_SUITES=$(printf '%s' "$EXPECTED_SUITES" | tr '
' ' ')  # V§17: line-boundary tokens failed the space-delimited case match

say()  { printf 'e2e: %s\n' "$*"; }
halt() { code=$1; shift; printf 'e2e: HALT %s\n' "$*" >&2; exit "$code"; }

# --- 0. locate the source repository (this script lives in <root>/tools) -----------
# Both `cd`s are checked. An unchecked `here=$(cd ... && pwd)` leaves `here` empty when the
# cd fails and the script carries on with `src=/` (req/08 §3 M-10) -- it died later anyway,
# but on the wrong line, and a gate that misreports where it broke teaches the wrong fix.
# -P so both sides of the comparison below are physical paths, as git's are.
here=$(cd -P -- "$(dirname -- "$0")" && pwd) || halt 10 "cannot enter the directory this script lives in"
src=$(cd -P -- "$here/.." && pwd)            || halt 10 "cannot enter the repository root above $here"

inside=$(git -C "$src" rev-parse --is-inside-work-tree)
rc=$?
if [ "$rc" -ne 0 ] || [ "$inside" != "true" ]; then
    halt 10 "$src is not a git work tree (rc=$rc)"
fi

# `is-inside-work-tree` answers yes for any directory BELOW a work tree, so a copy of this
# tree dropped inside somebody else's repository passes it. The script then read that
# repository's HEAD and printed three lines about a commit nobody asked for, before
# `git clone` failed on a path that is not a repository root (req/08 §3 M-11, run E10).
# The clone is taken from a root, so the root is checked here -- before anything is said.
top=$(git -C "$src" rev-parse --show-toplevel) || halt 10 "cannot read the work tree root at $src"
top=$(cd -P -- "$top" && pwd)                  || halt 10 "the work tree root $top is not reachable"
if [ "$top" != "$src" ]; then
    halt 10 "$src is not a repository root -- it sits inside the work tree at $top, whose commits are not this script's business"
fi

head=$(git -C "$src" rev-parse HEAD)         || halt 10 "cannot read HEAD"
branch=$(git -C "$src" rev-parse --abbrev-ref HEAD) || halt 10 "cannot read the branch"
dirty=$(git -C "$src" status --porcelain)    || halt 10 "cannot read the status"
if [ -z "$dirty" ]; then dirty_n=0; else dirty_n=$(printf '%s\n' "$dirty" | wc -l); fi

say "source  $src"
say "commit  $head on $branch"
say "excluded from the clone: $dirty_n uncommitted or untracked entries"

# --- 1. a scratch tree on ext4 -----------------------------------------------------
# ASM-01-1: the repo sits on a folder two operating systems write to. Nothing this
# script creates may land there, so a drvfs temp dir is a halt, not a warning.
work=$(mktemp -d "${TMPDIR:-/tmp}/glovrex-e2e-XXXXXX") || halt 11 "mktemp failed"
case "$work" in
    /mnt/*) halt 11 "temp dir landed on drvfs: $work" ;;
esac

cleanup() {
    # Capture the script's real exit status first: without this, the trap's last
    # command (rm) would replace a RED exit code with its own 0 (found live 2026-08-24:
    # a 101 run printed RED but exited 0 through this trap).
    rc_at_exit=$?
    if [ "${GLOVREX_E2E_KEEP:-0}" = "1" ]; then
        printf 'e2e: kept %s\n' "$work"
    else
        rm -rf -- "$work"
    fi
    exit "$rc_at_exit"
}
trap cleanup EXIT

# --- 2. clone: from here on, only what git handed over ------------------------------
# --no-hardlinks so the clone shares no inode with the source; the source is read, never
# touched. The clone is taken from the local repository, not from origin (see the gaps).
git clone --no-hardlinks --quiet -- "$src" "$work/glovrex" || halt 12 "git clone failed"
clone="$work/glovrex"

clone_head=$(git -C "$clone" rev-parse HEAD) || halt 12 "cannot read the clone's HEAD"
[ "$clone_head" = "$head" ] || halt 12 "the clone is at $clone_head, the source at $head"
tracked=$(git -C "$clone" ls-files | wc -l)
say "clone   $clone -- $tracked tracked files at $clone_head"

# A committed symlink walks through every path check that reads text. `req/outside -> /etc`
# has no `..`, no leading `/` and no drive prefix, yet `req/outside/passwd` reads a file
# this repository does not hold (req/08 V§4 N-2, run V5). git stores the link as mode
# 120000, so it rides into the clone -- which makes the index the place to catch it: on a
# Windows checkout with core.symlinks=false the link is an ordinary file on disk, but the
# mode git recorded is 120000 there too. The clone must hold no such entry at all.
links=$(git -C "$clone" ls-files -s -z | tr '\0' '\n' | awk -F'\t' '$1 ~ /^120000 / { print $2 }')
if [ -n "$links" ]; then
    say "RED    the clone carries a committed symlink (git mode 120000):"
    printf '%s\n' "$links" | while IFS= read -r link; do say "       $link"; done
    say "       a symlink is a path that leaves the repository without ever saying so, and"
    say "       git hands it to every clone -- so no path check downstream can be trusted"
    exit 18
fi

# --- 3. the subject, which git does NOT hold ----------------------------------------
# probes/doubt depends on ../../../Glovrex_Alpha/crates/* by path, deliberately (nothing
# is copied). That tree is outside the repository, so a clone cannot contain it: it has
# to be supplied from outside and said out loud. This is the largest gap in this audit.
alpha=${GLOVREX_ALPHA:-$(cd -- "$src/.." && pwd)/Glovrex_Alpha}
[ -d "$alpha/crates" ] || halt 13 "the subject is not at $alpha/crates (set GLOVREX_ALPHA)"
# An empty crates/ used to pass this gate and die later as `cargo exited 101`, which
# reports a setup mistake as a probe failure (req/08 §3 M-8, run E9). These four are the
# path dependencies of probes/doubt, so their absence is a missing subject, not a RED.
for crate in alpha-term alpha-fold alpha-ledger alpha-shell; do
    [ -f "$alpha/crates/$crate/Cargo.toml" ] ||
        halt 13 "the subject has no $crate ($alpha/crates/$crate/Cargo.toml is missing)"
done
ln -s -- "$alpha" "$work/Glovrex_Alpha" || halt 13 "cannot link the subject into $work"
say "subject $alpha (supplied from outside the clone -- git does not hold it)"

# --- 4. toolchain -------------------------------------------------------------------
cargo_bin=$(command -v cargo) || halt 14 "cargo is not on PATH -- run this under WSL with bash -lc"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.sg/target}"
case "$CARGO_TARGET_DIR" in
    /mnt/*) halt 15 "CARGO_TARGET_DIR is on drvfs: $CARGO_TARGET_DIR" ;;
esac
say "cargo   $cargo_bin -> $($cargo_bin --version)"
say "target  $CARGO_TARGET_DIR"

# --- 5. build and test the clone ----------------------------------------------------
# 5a. stage the audit fixture the clone cannot deliver (2026-08-15, K6 lane, req/167).
# `audit_p3_support::probe_server_path()` derives `<workspace>/target/debug/mcp_probe_server`
# from the workspace layout (CARGO_BIN_EXE is invisible across crates -- the fixture's own
# header says why), and a fresh clone holds no `target/` at all: with CARGO_TARGET_DIR
# pointing at the shared dir, nothing ever puts the binary there, and the three
# audit_p3_a2 probes die on a missing fixture instead of measuring gx. This has been true
# for every clone since the audit_p3 suites entered EXPECTED_SUITES; it surfaced when the
# K6 lane ran this script start to finish. The binary is built from the clone's own
# sources (warm deps via the shared target -- the same declared warmness as gap note 3)
# and copied to where the fixture looks, exactly what tools/audit_p3_run.sh stage 1 does
# on the working tree.
# 🔴 **req/817 -- the fixture's crate is not in every tree this script can be run in.**
#
# `gx-mcp-wire` is one of the four crates `req/789` §3 holds private, so it is absent from the
# public distribution -- and `tools/e2e.sh` itself IS in the public sync set. Before this guard the
# script halted 12 on a public clone at exactly this line, which made the published repository ship
# an end-to-end script that could not reach its own first test. (Found by running it in a
# public-shaped tree, not by reading the sync manifest -- the same way req/818 F1 was found.)
#
# The guard is a skip and it is loud: a tree that HAS the crate must still build the fixture, and a
# halt there is still a halt. Only a tree that genuinely does not carry the crate skips it, and it
# says which suites go unmeasured as a result rather than reporting a green that covered less.
if [ -f "$clone/crates/gx-mcp-wire/Cargo.toml" ]; then
    say "staging mcp_probe_server (audit_p3 fixture) into the clone's target/debug"
    ( cd -- "$clone" && cargo build --locked -p gx-mcp-wire --features probe-bin --bin mcp_probe_server ) \
        > "$work/fixture-build.log" 2>&1 || halt 12 "the audit fixture did not build -- log: $work/fixture-build.log"
    mkdir -p -- "$clone/target/debug"
    cp -- "$CARGO_TARGET_DIR/debug/mcp_probe_server" "$clone/target/debug/" \
        || halt 12 "the audit fixture did not land in the clone's target/debug"
else
    say "SKIP mcp_probe_server (audit_p3 fixture): this tree carries no crates/gx-mcp-wire"
    say "     -- it is private (req/789 §3), so the audit_p3_a2/a3/a6 probes that need a real MCP"
    say "     server are NOT measured in this run. Everything else below still is."
fi

log="$work/cargo-test.log"
say "running cargo test --workspace --locked --no-fail-fast in the clone"
# Entering the clone is its own step. It used to sit inside the pipeline as
# `( cd "$clone" && cargo test ... )`, where a failed cd landed in PIPESTATUS[0] and was
# printed as `cargo exited N` -- a setup failure reported as a probe failure, the same
# misattribution M-8 fixed at the subject gate (req/08 §3 M-9). Now only cargo's own status
# can reach PIPESTATUS. Everything after this point uses absolute paths.
cd -- "$clone" || halt 19 "cannot enter the clone at $clone"
# 🔴 req/635 box M-2 / req/38 §394 M-2 plan A (report req/644): `permissions_do_not_bind`
# (r41_presence_unification.rs, r43_presence_and_head.rs) skips silently under euid 0 and cargo's
# own summary still reads "0 ignored" either way (req/621 §3-2) -- a `println!` alone is not
# `git grep`-able. Both suites now append one line per skip to this file when GX_TEST_SKIP_CARRIER
# is set; clearing it here makes each e2e run's reading exact rather than accreting across runs.
skip_carrier="$work/skip_carrier.log"; rm -f -- "$skip_carrier"; export GX_TEST_SKIP_CARRIER="$skip_carrier"
# 🔴 `--no-fail-fast` (req/477, req/478 §4): without it cargo stops launching test targets after
# the first one that fails, so a single red binary truncates the floor -- the count then measures
# "how far cargo got", not the workspace. That is a completeness defect in a floor whose whole job
# is to make a regression visible: the known-red set (postgres without a DSN, audit_p3) would hide
# every suite ordered behind it, and which suite that is depends on compile order, so the same tree
# could report two different floors on two runs. With the flag every target runs and `rc` still
# reports failure, so nothing is made to look green that was not.
cargo test --workspace --locked --no-fail-fast 2>&1 | tee "$log" | tail -n 20
rc=${PIPESTATUS[0]}

# The three readings below scan cargo's own output, which holds only because libtest
# captures a passing test's stdout by default and because rc is judged first below.
# Introducing --nocapture removes that premise: a probe's own print of a `test result:`
# line would then be counted, so this scan would have to be redesigned (req/08 V§11).
passed=$(awk '/^test result: ok\./ {s+=$4} END {print s+0}' "$log")
suites=$(awk '/^test result:/ {n++} END {print n+0}' "$log")

# Which suites actually ran, taken from cargo's own `Running tests/<name>.rs (...)` lines
# rather than from the file listing -- a test file that exists but never ran would still
# be counted by `ls`.
ran=$(sed -n 's|^[[:space:]]*Running tests/\([A-Za-z0-9_]*\)\.rs .*|\1|p' "$log" | sort -u | tr '\n' ' ')
missing_suites=''
for want in $EXPECTED_SUITES; do
    case " $ran " in *" $want "*) ;; *) missing_suites="$missing_suites $want" ;; esac
done
extra_suites=''
for got in $ran; do
    case " $EXPECTED_SUITES " in *" $got "*) ;; *) extra_suites="$extra_suites $got" ;; esac
done

# req/635 box M-2 (shared, required): "the inversion of the 0-ignored assert" -- a skip is neither
# `ignored` nor a greppable line; only this carrier holds it. A non-zero count is the measurement that
# the run environment's euid does not bind, and since cargo's own summary stays "ok" as before, no one
# sees it unless this read is added.
skip_lines=0; [ -s "$skip_carrier" ] && skip_lines=$(grep -c . -- "$skip_carrier")
printf '\n'
if [ "$rc" -ne 0 ]; then
    say "RED    cargo exited $rc -- full log: $log (re-run with GLOVREX_E2E_KEEP=1 to keep it)"
elif [ "$passed" -lt "$MIN_PROBES" ] || [ "$suites" -lt "$MIN_SUITES" ]; then
    say "RED    cargo exited 0 but only $passed probes over $suites suites ran, under the"
    say "       floor of $MIN_PROBES over $MIN_SUITES -- a suite that did not run is not a suite that passed"
    rc=16
elif [ -n "$missing_suites" ] || [ -n "$extra_suites" ]; then
    say "RED    cargo exited 0 and $passed probes over $suites suites ran, but they are not the"
    say "       declared suites -- the count held while the question changed"
    [ -n "$missing_suites" ] && say "       never ran:$missing_suites"
    [ -n "$extra_suites" ] && say "       ran but undeclared:$extra_suites"
    say "       expected exactly: $EXPECTED_SUITES"
    rc=17
elif [ "$skip_lines" -gt 0 ]; then
    say "RED    cargo exited 0 and $passed probes over $suites suites ran, but the euid-guard skip"
    say "       carrier holds $skip_lines line(s) (r41_presence_unification.rs /"
    say "       r43_presence_and_head.rs, req/621 §3-2's fail-open) -- an arm that measured nothing"
    say "       folded into the same \"passed\"/\"ok\" cargo prints for one that measured its door."
    say "       carrier: $skip_carrier"
    rc=22
else
    say "GREEN  $passed probes over $suites suites, from the clone alone (rc=0), 0 skip-carrier lines"
fi

# --- 5b. the SDK, from the same clone -----------------------------------------------
# 🔴 **R12 / `req/242` M-04.** `sdk/typescript` is a published package (`tracefold` on npm) whose
# `gx_code` vocabulary, `InverseStatus` union and `ProblemDetail.retry_after_ms` are copies of
# things this repository declares in Rust. Until this stage, no floor ran a line of it: `e2e.sh`
# executed no `npm` and no `node`, and said so at line ~1054 ("SDK e2e +1 (`node --test`, outside
# this floor)"). "Outside this floor" is where a probe goes stale, which is exactly what the audit
# found -- `src/` held twenty-one codes, the `dist/` on the disk held thirteen, and the census
# passed because it read the `dist/`.
#
# Two toolchains this stage needs and the clone cannot supply, named the way the postgres DSN is
# (a missing environment must halt with its own sentence, never skip quietly -- req/29 §4):
#   * `node` (>= 18) on PATH. The repository pins none; install one however this machine does.
#   * `wasm-bindgen` matching `Cargo.lock`'s `wasm-bindgen` (0.2.127). `sdk/typescript/src/wasm-gen/`
#     is `.gitignore`d, so a fresh clone has to regenerate it from `sdk/wasm-verify` before `tsc`
#     can compile `src/verify.ts`.
# And one thing this stage does that no other stage does: `npm ci` **reaches the network**. That is
# a new dependency for this script and it is written into the gaps below rather than left implied.
say ""
command -v node > /dev/null 2>&1 || halt 20 "node is not on PATH -- stage 5b runs the SDK's own suite (req/242 M-04). Install Node >= 18 and re-run"
command -v wasm-bindgen > /dev/null 2>&1 || halt 20 "wasm-bindgen is not on PATH -- sdk/typescript/src/wasm-gen/ is gitignored and the clone must regenerate it (cargo install wasm-bindgen-cli --version 0.2.127)"
say "sdk     node $(node --version), npm $(npm --version), $(wasm-bindgen --version)"

sdk_log="$work/sdk-test.log"
if ! ( cd -- "$clone" && bash sdk/wasm-verify/build.sh ) > "$work/wasm-build.log" 2>&1; then
    say "RED    the SDK's wasm glue did not build -- log: $work/wasm-build.log"
    rc=21
elif ! ( cd -- "$clone/sdk/typescript" && npm ci ) > "$work/npm-ci.log" 2>&1; then
    say "RED    npm ci failed in the clone -- log: $work/npm-ci.log"
    rc=21
else
    ( cd -- "$clone/sdk/typescript" && npm test ) > "$sdk_log" 2>&1
    sdk_rc=$?
    # Node <= 22 prints TAP ("# pass 18"); Node >= 23's default spec reporter prints
    # an information glyph ("ℹ pass 18"). Both are the runner's own summary, so both
    # are accepted -- a floor that reads only one dialect calls a passing suite "0 passing"
    # (measured on Node v24.18.1, 2026-08-17, all 18 passing yet RED).
    sdk_pass=$(awk '$1=="pass"||$2=="pass"{print $NF}' "$sdk_log" | grep -E '^[0-9]+$' | tail -1)
    sdk_fail=$(awk '$1=="fail"||$2=="fail"{print $NF}' "$sdk_log" | grep -E '^[0-9]+$' | tail -1)
    sdk_skip=$(awk '$1=="skipped"||$2=="skipped"{print $NF}' "$sdk_log" | grep -E '^[0-9]+$' | tail -1)
    : "${sdk_pass:=0}" "${sdk_fail:=0}" "${sdk_skip:=0}"
    if [ "$sdk_rc" -ne 0 ] || [ "$sdk_fail" -ne 0 ]; then
        say "RED    the SDK suite exited $sdk_rc with $sdk_fail failing -- log: $sdk_log"
        rc=21
    elif [ "$sdk_pass" -lt "$MIN_SDK_PASS" ] || [ "$sdk_skip" -gt "$MAX_SDK_SKIP" ]; then
        # A suite that stopped running is not a suite that passed, and a skip is how it stops
        # quietly: the seven this floor allows are the `GX_BINARY`-gated ones, which the clone
        # deliberately does not set.
        say "RED    the SDK suite ran $sdk_pass passing and $sdk_skip skipped, against a floor of"
        say "       $MIN_SDK_PASS passing and at most $MAX_SDK_SKIP skipped -- log: $sdk_log"
        rc=21
    else
        say "GREEN  SDK $sdk_pass passing, $sdk_fail failing, $sdk_skip skipped (node --test, from the clone)"
    fi
fi

# --- 6. what this run did not look at -----------------------------------------------
cat <<'GAPS'

e2e: NOT COVERED BY THIS SCRIPT
  1. lake build (req/lean, GxSpec.Core). elan and lake exist only on the Windows side,
     so no Lean code was compiled here. The other half must be run there:
       powershell> cd $env:USERPROFILE\OneDrive\Desktop\glovrex\req\lean
       powershell> & "$env:USERPROFILE\.elan\bin\lake.exe" build
  2. Glovrex_Alpha. It is a path dependency outside the repository: supplied by this
     script, not delivered by the clone. A fresh machine with the clone alone cannot
     reproduce this run. Alpha's own test suite was not run either.
  3. CARGO_TARGET_DIR is shared with ordinary working-tree builds, so this was a warm
     compile, not a cold one. Export a fresh CARGO_TARGET_DIR to close that gap.
  4. Only the committed HEAD of the current branch was tested, cloned from the LOCAL
     repository -- not from origin. Whether origin holds the same commit is untested.
  5. No clippy, no fmt, no Kani, no dependency audit, no benchmark; benches are compiled
     by cargo test only as far as `--locked` requires, and none were run. Those stages are
     tools/ci.sh, which runs against the working tree and not against a clone.
  6. The probes themselves are only as good as what they assert; a green run means the
     assertions held, not that the subject is correct.
  7. 🔴 R12: stage 5b's `npm ci` REACHES THE NETWORK (registry.npmjs.org). Every other stage
     of this script runs offline. A machine without the registry cannot run this floor.
  8. 🔴 R12: seven of the SDK's tests skip, and the floor allows exactly that many. They are
     the ones gated on `GX_BINARY` (an end-to-end walk against a real `gx`), which this
     script does not set: the clone's binary is built for the cargo stage, not exported to
     the SDK. What stage 5b measures is the SDK against this repository's *source* -- the
     `gx_code` census, the endpoint parity, the offline receipt verification and the
     tamper arms -- and not the SDK against a running server.
GAPS

exit "$rc"
