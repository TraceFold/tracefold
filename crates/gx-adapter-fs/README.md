<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-adapter-fs

**The filesystem SubstrateAdapter: single-file whole replacement, lexical locators.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The filesystem implementation of the substrate boundary. A locator is an absolute path; `apply` creates, replaces and removes whole files, and `invert` puts the previous whole file back. |
| **What it guarantees** | All seven adapter contracts are implemented — no method answers "unimplemented", so the crate is both conformant and complete against the shared harness. The delta grammar is deliberately narrow: one operation, one whole file, no directory creation, no mode or owner change, and no symbolic link followed by the adapter's own choice. |
| **What it refuses to do** | It has **no allow-list, no root and no chroot**. Any absolute path this process can write is a path this adapter will write; the only bound that exists is the process's own credentials plus that delta grammar. It performs no confinement of its own — what stands between an intent and a file is the gate, not this crate. The kernel still follows a symbolic link when opening a path, which this crate does not close. And the conformance run behind "all seven" is a fixture on a temporary filesystem, in one thread, over files it created itself: it is not a claim about filesystems in general, about crash durability, about concurrency, or about the other adapters. |
| **How it is checked** | [`tests/`](tests) — [`conformance.rs`](tests/conformance.rs) runs the shared harness, [`locator_normalisation.rs`](tests/locator_normalisation.rs) the lexical rule, [`plan_purity.rs`](tests/plan_purity.rs) that planning writes nothing, [`invert_ceiling.rs`](tests/invert_ceiling.rs) / [`forward_ceiling.rs`](tests/forward_ceiling.rs) the declared upper bounds, [`fault_injection.rs`](tests/fault_injection.rs) and [`apply_durability.rs`](tests/apply_durability.rs) what happens when the write goes wrong. |

---

## Where it sits

An implementation of the trait in [`gx-substrate`](../gx-substrate), measured by
[`gx-substrate-conformance`](../gx-substrate-conformance), and reached only through
[`gx-gate`](../gx-gate)'s decision. It is a sibling of
[`gx-adapter-git`](../gx-adapter-git) and [`gx-adapter-mcp`](../gx-adapter-mcp).

## Learn more

- [`src/lib.rs`](src/lib.rs) — the bounds that exist and the ones that deliberately do not.
- [`policies/fs/`](../../policies/fs) — the shipped rule pack for this substrate, with its scenarios.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — the writes this project cannot judge from the inside.
