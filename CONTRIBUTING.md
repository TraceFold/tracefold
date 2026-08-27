# Contributing to Tracefold

Thank you for your interest in contributing to Tracefold! Tracefold is built in Rust with formal mathematical guarantees (Lyapunov convergence, pre-commit inverse calculus, and DSSE receipts).

## Quickstart in 60 Seconds

1. **Prerequisites**: Ensure you have Rust 1.80+ installed (`rustup update stable`).
2. **Clone and Build**:
   ```bash
   git clone https://github.com/TraceFold/tracefold.git
   cd tracefold
   cargo build --all-targets
   ```
3. **Run All Tests**:
   ```bash
   cargo test --all
   ```

## Where to Start?

Check out our labeled issues:
- [`good first issue`](https://github.com/TraceFold/tracefold/labels/good%20first%20issue): Well-scoped tasks ideal for newcomers (e.g. docs, bindings, quickstarts).
- [`help wanted`](https://github.com/TraceFold/tracefold/labels/help%20wanted): High-priority community contributions (e.g. Web UI, Python PyO3 bindings, WASM target).

## How to Add a New Tool Invariant

All tool invariants live in `crates/gx-core/src/invariants/`:
1. Implement the `ToolInvariant` trait for your target tool (e.g. `sqlite_query`, `docker_exec`).
2. Define the pre-condition check (`verify_pre()`) and post-condition assertion (`verify_post()`).
3. Construct the exact pre-commit inverse payload (`generate_inverse()`).
4. Add unit tests and fuzz vectors covering edge cases.

## Pull Request Guidelines

- Keep PRs focused on a single feature, bugfix, or doc improvement.
- Ensure all tests pass (`cargo test --all`) and formatting matches (`cargo fmt --check`).
- PRs modifying core calculus must include deterministic test vectors.
