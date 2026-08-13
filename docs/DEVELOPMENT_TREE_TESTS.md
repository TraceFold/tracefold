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

## This repository's floor

2026-08-13 measured, `cargo test --workspace` on this staging tree (WSL, `rustc`/`cargo` 1.97.1): **1,160 passed / 216 suites, 0 failed.** ("Suites" here counts every `cargo test` harness invocation: one `unittests` run per crate that has one, one run per file under `tests/`, and one `Doc-tests` run per crate.) This is the number for this public repository as it stands today, not the development tree's 1,370/247.
