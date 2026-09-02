<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-substrate-conformance

**The adapter contract harness: seven contracts and the laws the rulings added.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | One adapter-independent test harness, sixteen obligations: seven contracts every `SubstrateAdapter` must satisfy, plus nine laws added by later rulings. An adapter inherits the whole set by calling `run_all` once from its own `#[test]`, so no adapter author writes their own version of these checks. |
| **What it guarantees** | Three questions are kept **separate**, and that separation is the point of the crate. `conformant` = zero failures: nothing measured contradicted an obligation. `complete` = zero unmeasured: every obligation had a subject to run against. `meets_51_7` = both. A method an adapter has not implemented yet reports as **not supplied** — never a silent pass, and never a failure. Not-checked and checked-and-clean are different facts here, exactly as they are in the product. |
| **What it refuses to do** | It measures adapters against the substrate boundary and nothing else. It is **not** a claim about receipt or wire-format conformance, and it is not the differential-vector suite that checks the Rust implementation against the Lean model — that is a different mechanism one word away in name and shares no code with this one. It adds no obligation of its own: `contracts.rs` and `laws.rs` are where the sixteen live. |
| **How it is checked** | The harness's own standing negative control is [`tests/broken_fixture.rs`](tests/broken_fixture.rs) — eighteen deliberate flaws, one obligation broken at a time, each asserted to come back as a failure (or, for the one flaw meaning "not implemented yet", as not-supplied). Without it, an entry point that only ever printed green would be indistinguishable from one that ignores its own results. [`tests/contracts_seven.rs`](tests/contracts_seven.rs) and [`tests/laws.rs`](tests/laws.rs) cover the obligations themselves, [`tests/opacity.rs`](tests/opacity.rs) that the harness cannot see a delta's payload, [`tests/residual.rs`](tests/residual.rs) what is left unmeasured. |

---

## Where it sits

Beside [`gx-substrate`](../gx-substrate), which declares the boundary this crate measures against.
The three shipped adapters each run it from their own test:
[`gx-adapter-fs`](../gx-adapter-fs) in [`conformance.rs`](../gx-adapter-fs/tests/conformance.rs),
[`gx-adapter-git`](../gx-adapter-git) in [`git_conformance.rs`](../gx-adapter-git/tests/git_conformance.rs),
[`gx-adapter-mcp`](../gx-adapter-mcp) in [`mcp_conformance.rs`](../gx-adapter-mcp/tests/mcp_conformance.rs).
It is a test-only crate and is not a publish target.

## Running it

```bash
cargo test -p tracefold-adapter-fs --test conformance
cargo test -p tracefold-adapter-git --test git_conformance
cargo test -p tracefold-adapter-mcp --test mcp_conformance
cargo test -p gx-substrate-conformance --test broken_fixture   # the negative control
```

Each adapter's test prints one summary line carrying the three verdicts above, so a run tells you
not only whether anything failed but whether everything was actually measured. All three adapters
are expected green with no external service running.

An adapter that needs a live server to be measured is reported as **not run** rather than folded
into either "conformant" or "failed" — an environment that has not been set up is not a defect in
the code, and this crate refuses to let the two look the same.

## Learn more

- [`src/lib.rs`](src/lib.rs) — the report shape and the three questions, from the crate's own side.
- [`src/contracts.rs`](src/contracts.rs) / [`src/laws.rs`](src/laws.rs) — the sixteen obligations.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — what passing all sixteen does not tell you.
