<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# policies/

Cedar policy packs. Each pack is a rule file plus the scenario file that proves the rules still
say what they were written to say. [`gx-gate`](../crates/gx-gate) embeds them at build time and
evaluates them for every transformation.

---

| Pack | Rules | Scenarios | What the rules refuse |
| :--- | :--- | :--- | :--- |
| **fs** | [`fs/deny-etc.cedar`](fs/deny-etc.cedar) | [`fs/scenarios.json`](fs/scenarios.json) | Writes into system configuration paths, for the [filesystem adapter](../crates/gx-adapter-fs). |
| **git** | [`git/deny-nonbranch-refs.cedar`](git/deny-nonbranch-refs.cedar) | [`git/scenarios.json`](git/scenarios.json) | Changes aimed at references that are not branches, for the [git adapter](../crates/gx-adapter-git) — an adapter whose whole reversibility argument rests on a branch being resettable. |
| **mcp** | [`mcp/deny-etc-resources.cedar`](mcp/deny-etc-resources.cedar) | [`mcp/scenarios.json`](mcp/scenarios.json) | Tool calls aimed at system-configuration resources, for the [MCP adapter](../crates/gx-adapter-mcp). |

Every pack ships its `scenarios.json` beside its rules, because a pack handed to a third party
without the cases it is supposed to decide is a pack nobody can check. Two tests in the workspace
hold the tree to that: one asserts the locator shapes each pack claims, the other runs every
shipped pack against its own scenario file.

---

## What a pack does and does not decide

A policy sees the locator, the actor, the change context, the ordering, whether an inverse could
be built, and the evidence. It does **not** see the contents of the change — a delta's payload is
opaque below the adapter boundary, by construction. So a rule can say *this position must not be
written by this actor*; it cannot say *this text must not be written*.

A gate that was handed no packs at all answers with an error rather than admitting everything: an
empty policy directory and a working deployment must not look the same.

Policies encoding the wrong intent are enforced faithfully. Whether the intent is right is outside
what this project can check — see [`docs/LIMITS.md`](../docs/LIMITS.md).

## Adding a pack

A pack is a directory holding one or more `.cedar` files and one `scenarios.json`. Both are
required; the scenario file is what lets anyone other than the author confirm the rules behave.
The three packs above are the working examples to copy the shape from.
