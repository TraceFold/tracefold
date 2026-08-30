<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-adapter-git

**The git SubstrateAdapter: a file on a branch, moved by a commit, undone by a reference reset.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The git implementation of the substrate boundary. The **object** is the file at one path, on one branch, in one repository. The surrounding state that could interfere with it — the **scope** — is the branch, digested by the commit it points at. A locator spells all three parts: repository path, reference, file path. |
| **What it guarantees** | All three locator parts are required; a spelling missing a separator is reported as *not a position* (`Error::NotAPosition`), rather than as a position that then fails to apply. Two locators name one position exactly when normalisation maps them to the same string, and that normalisation is **purely lexical** — a function of the text, performing no repository read at all. |
| **What it refuses to do** | Because the scope is the whole branch, **two changes to two different files on one branch conflict**, and one of them waits. That is the true shape of a branch, not a limitation to be lifted later, and it is stated rather than hidden. The underlying git library is used through its public API only: its implementation is neither read nor reproduced here, and the delta grammar, the locator normalisation and the contract quantifiers are this project's own. |
| **How it is checked** | [`tests/`](tests) — [`git_conformance.rs`](tests/git_conformance.rs) runs the shared harness, [`git_commutation.rs`](tests/git_commutation.rs) the branch-scope conflict rule, [`git_delta.rs`](tests/git_delta.rs) the delta grammar, [`git_plan_purity.rs`](tests/git_plan_purity.rs) that planning writes nothing, [`h2_normalised_before_the_gate.rs`](tests/h2_normalised_before_the_gate.rs) that the gate never sees an un-normalised spelling. |

---

## Where it sits

An implementation of the trait in [`gx-substrate`](../gx-substrate), measured by
[`gx-substrate-conformance`](../gx-substrate-conformance), and reached only through
[`gx-gate`](../gx-gate)'s decision. It is a sibling of
[`gx-adapter-fs`](../gx-adapter-fs) and [`gx-adapter-mcp`](../gx-adapter-mcp).

## Learn more

- [`src/lib.rs`](src/lib.rs) — object, scope, locator grammar, and what the branch scope costs.
- [`policies/git/`](../../policies/git) — the shipped rule pack for this substrate, with its scenarios.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — the changes this project cannot judge from the inside.
