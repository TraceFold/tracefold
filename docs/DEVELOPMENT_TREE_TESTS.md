# Development-tree-only tests

The following 17 doc-conformance suites are not included in this repository. Each one measures agreement between the code and a private requirements/specification corpus (`req/spec/...`) or a private conformance probe tree (`probes/doubt`) that lives in the development tree this repository is cut from, not in this public repository. A test that reads a canon this tree does not contain would either fail to find the file or report against nothing, so it is withheld rather than shipped red.

They run, and pass, in the development tree where the canon exists. Removing them here does not remove any coverage this repository claims: the acceptance criteria they check are internal-process instruments, not properties of the shipped code, and the code paths they exercise are still covered by the suites that remain (each of the 16 canon-reading files pairs with sibling suites in the same crate that test the same module against fixtures and the source itself, not against `req/spec/`).

The development tree's own floor at milestone M7 is 1,370 probes across 247 suites (see the technical report, §5, for the conditions attached to that number). That number is **not** the floor of this public repository; see below for this repository's own measured floor.

## Excluded files

Sixteen match a direct reference to `req/spec/` or `../../../req` (an `include_str!`, `std::fs::read_to_string`, or a `read_repo`/`spec`/`read` helper that resolves to a path under `req/spec/`), verified by reading each file's runtime code path rather than by the grep alone (two files that only *cite* a `req/spec/...` path in a doc comment — `crates/gx-core/tests/ac_001.rs` and `crates/gx-substrate/tests/ac_046.rs` — do not read canon at runtime and are **not** excluded; they ship in this repository):

- `crates/gx-witness/tests/ac_018.rs`
- `crates/gx-witness/tests/pae_golden.rs`
- `crates/gx-substrate/tests/adapter_spec.rs`
- `crates/gx-substrate-conformance/tests/harness_shape.rs`
- `crates/gx-core/tests/enforcement_axes.rs`
- `crates/gx-gate/tests/error_vocabulary.rs`
- `crates/gx-gate/tests/gate_input_spec.rs`
- `crates/gx-gate/tests/gate_shape.rs`
- `crates/gx-gate/tests/verdict_identity.rs`
- `crates/gx-engine/tests/ac_045.rs`
- `crates/gx-engine/tests/escrow_types.rs`
- `crates/gx-engine/tests/engine_shape.rs`
- `crates/gx-engine/tests/journal_vocabulary.rs`
- `crates/gx-engine/tests/lifecycle_states.rs`
- `crates/gx-engine/tests/state_machine_coverage.rs`
- `crates/gx-cli/tests/exit_map.rs`

One file does not match that grep — it names its citations as `req/29`, `req/08`, and `42 §…` in prose rather than as a `req/spec/` path — but fails for the same underlying reason: it hard-asserts that a `probes/` directory exists at the workspace root (`assert!(dir.is_dir(), ...)`, by design, per its own comment: "a check that cannot find the tree it audits must fail, not pass"). `probes/doubt` is the private conformance-probe crate this workspace's `Cargo.toml` already documents as outside this repository. Without it, this file panics rather than measuring anything:

- `crates/gx-canon/tests/ac_014.rs`

Two other files reference a `probes/` path (`crates/gx-substrate-conformance/tests/print_consumers.rs`, `crates/gx-gate/tests/pack_embedding.rs`) but degrade gracefully when it is absent (`fs::read_dir` behind an `Ok(..) else { continue }`, scanning only `crates/` when `probes/` is not there); they are real, passing tests in this repository and are not excluded.

## 2026-08-25 update: the list above is now machine-derived, and three files were added

The 17 files above were a hand-maintained list, and hand-maintained lists decay silently: this
repository's sync tooling (`tools/pub_sync_dryrun.sh`, in the development tree this repository is
cut from) now derives the exclusion set by grep at every sync -- any tracked file under a
`tests/`/`test/` directory whose *non-comment* lines name a `req/spec/...` path (matching
`include_str!`, `fs::read_to_string`, `read_repo`, `readFileSync`, or `readRepoFile` literals; a
comment-only citation, like the two files noted above, does not count) -- unioned with the 17-file
hand list as a floor that is never shrunk. The derivation caught one file the hand list had missed
and this document had never named, plus two equivalent cases in the TypeScript SDK the Rust-only
hand list could not have named:

- `crates/gx-log/tests/nfr_027.rs` -- reads `req/spec/30-requirements/33-non-functional.md` and
  `35-open-questions.md` at runtime. It post-dates the original 17-file list and was never added to
  it by hand; the derivation now finds it every run.
- `sdk/typescript/test/audit_m9_p4_independent_parity.test.mjs` -- an independent re-parse of
  `req/spec/40-architecture/44-api-spec.md` (the M9 audit lane's cross-check of AC-P4-1, deliberately
  not reusing the SDK's own parser -- see the file's own header comment for why).
- `sdk/typescript/test/endpoint_parity.test.mjs` -- does not name `req/spec` in its own text; it
  imports and calls `specifiedEndpointsFromSpec()` from `sdk/typescript/testlib/support.mjs`, which
  performs the read. This is a transitive case a plain text grep on the test file's own content
  cannot see (the earlier two SDK findings, from the ruling this section follows through on, named
  the direct case but not this one hop further out).

`sdk/typescript/testlib/support.mjs` itself **ships** (it is not excluded): six other shipped tests
(`gx_code_census.test.mjs`, `quickstart.test.mjs`, `gui_probe_runs.test.mjs`,
`gui_probe_import_boundary.test.mjs`, `inverse_status_vocabulary_parity.test.mjs`,
`server_health_vocabulary_parity.test.mjs`) import other functions from the same file
(`repoRoot`/`sdkRoot`/`readRepoFile`) against paths this repository does contain, and excluding the
whole file would break all six. `specifiedEndpointsFromSpec()` is dead code in this repository after
the two files above are withheld: nothing shipped calls it, so it never runs and never fails; it
would throw a clean "file not found" if anything ever did.

`crates/gx-canon/tests/ac_014.rs` (the probes/doubt case, above) is not caught by this grep -- it
references `probes` as a bare directory name, not a `req/spec` path -- so it stays excluded only
because it is still on the hand-list floor. The total is now **20 files** (19 grep-derived + 1
hand-list-only).

## This repository's floor

2026-08-13 measured, `cargo test --workspace` on this staging tree (WSL, `rustc`/`cargo` 1.97.1): **1,160 passed / 216 suites, 0 failed.** ("Suites" here counts every `cargo test` harness invocation: one `unittests` run per crate that has one, one run per file under `tests/`, and one `Doc-tests` run per crate.) This is the number for this public repository as it stands today, not the development tree's 1,370/247.

🔴 **2026-08-31 staleness flag (public-docs audit, Owner #146).** The 2026-08-13 figure above is
now over two weeks old and has not been re-measured on this staging tree since. It is stale by
construction, not by a found defect: the development tree's own floor has grown from the
1,370/247 named above to a figure on the order of **2,700 passed / 470+ suites** as of
2026-08-29/30 (`req/00-LOOP_STATE_2026-08-18.md`, 2026-08-30 tick; `crates/gx-witness` commit
`0bba4837`, "floor 2684/474"; this is the private tree's number and is **not** the public
repository's own count — the two have always differed, as the paragraph above already says).
Dozens of test files named throughout `docs/LIMITS.md`'s changelog (the `r3*`/`r4*`/`g8`/`g9`
suites, 2026-08-15 through 2026-08-29) postdate this measurement and are not reflected in
**1,160 / 216**. This page's own convention (re-derive from the commands given, do not trust the
table) applies to itself: the honest reading of this section today is "at least 1,160 / 216, most
likely higher," not "1,160 / 216." A fresh `cargo test --workspace` on the public staging tree is
the correction this note stands in for; nobody has run and recorded one since 2026-08-13.
