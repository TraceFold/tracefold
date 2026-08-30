<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-gate

**Cedar policy evaluation, invariant registry and verdict composition.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The predicate that decides whether a transformation is admissible. It evaluates the Cedar policy set, runs every registered invariant, and composes the two into one verdict. |
| **What it guarantees** | If either the policy set or an invariant refuses, the gate refuses. A gate that was never given a policy set still answers with an error, because an empty policy directory and a working deployment must not look the same. |
| **What it refuses to do** | It is **not** the state machine: a verdict says what is true of a transformation; what a system *does* about that — refuse the commit, record it anyway, wait for a human — is decided one layer up. It also **cannot see the change it is judging**; a delta's payload is opaque below the adapter, so a policy may reason only over the locator, the actor, the change context, the order, whether an inverse exists, and the evidence. And admissibility does **not** compose for an arbitrary policy set: one policy forbidding any transformation that touches two given paths together is a standing counterexample, and this crate does not claim otherwise. |
| **How it is checked** | [`tests/`](tests) — [`false_admit.rs`](tests/false_admit.rs) with its own vector directory for the failure that matters most, [`policy_determinism.rs`](tests/policy_determinism.rs) and [`deny_order.rs`](tests/deny_order.rs) for evaluation order, [`verdict_meet.rs`](tests/verdict_meet.rs) / [`verdict_order.rs`](tests/verdict_order.rs) for composition, [`pack_embedding.rs`](tests/pack_embedding.rs) and [`shipped_set.rs`](tests/shipped_set.rs) for the packs that actually ship. |

---

## Where it sits

Above [`gx-core`](../gx-core), [`gx-canon`](../gx-canon) and [`gx-witness`](../gx-witness), and
below [`gx-engine`](../gx-engine), which asks it for a verdict and then acts on one. The shipped
rule sets it evaluates live in [`policies/`](../../policies).

## Learn more

- [`src/lib.rs`](src/lib.rs) — what a verdict is, and what it deliberately is not.
- [`policies/`](../../policies) — the three policy packs that ship, with their conformance scenarios.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — including the boundaries a policy cannot be written to close.
