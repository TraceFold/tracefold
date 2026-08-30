<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# gx-adapter-mcp

**The MCP SubstrateAdapter: a tool call that cannot reach a server except through an admitted apply.**

Part of [Tracefold](../../README.md) — the workspace that holds a checked inverse for an agent's
change before it lands.

---

| Dimension | This crate |
| :--- | :--- |
| **What it is** | The Model Context Protocol implementation of the substrate boundary. The **object** is the resource at one URI on one server. A locator spells the server endpoint URI and the resource URI, both required, with the endpoint carrying a scheme. |
| **What it guarantees** | Nothing in this workspace can call a tool without going through one admitted `apply` — that is the property this crate was built and measured to hold. The footprint of a change is the **whole server**, so two changes on one server are treated as conflicting and one of them waits. |
| **What it refuses to do** | A tool call whose effect lands on a resource *other* than the one its transformation is about is **invisible** to the fingerprint: the fingerprint reads the object, and the object is not where the effect went. What is offered against that is serialisation, not detection — nothing else on that server runs beside it. Real detection would need the server to tell a proxy what a tool touches, which no part of the protocol does. No JSON-RPC framing ships in this crate, so "the proxy speaks MCP" is **not** among the things measured here. A `#` inside the resource URI is refused, because that character is reserved for the fragment and the grammar could not then find its own separator. |
| **How it is checked** | [`tests/`](tests) — [`mcp_conformance.rs`](tests/mcp_conformance.rs) runs the shared harness, [`mcp_commutation.rs`](tests/mcp_commutation.rs) the server-wide footprint, [`mcp_plan_purity.rs`](tests/mcp_plan_purity.rs) that planning calls nothing, [`undo_cas_mcp.rs`](tests/undo_cas_mcp.rs) the compare-and-set on undo, [`h2_normalised_before_the_gate.rs`](tests/h2_normalised_before_the_gate.rs) that the gate never sees an un-normalised locator, and the catalogue tests against recorded fixtures in [`tests/fixtures/`](tests/fixtures). |

---

## Where it sits

An implementation of the trait in [`gx-substrate`](../gx-substrate), measured by
[`gx-substrate-conformance`](../gx-substrate-conformance), and reached only through
[`gx-gate`](../gx-gate)'s decision. It also consumes [`gx-engine`](../gx-engine) and
[`gx-witness`](../gx-witness), and is a sibling of [`gx-adapter-fs`](../gx-adapter-fs) and
[`gx-adapter-git`](../gx-adapter-git).

## Learn more

- [`src/lib.rs`](src/lib.rs) — object, footprint, locator grammar, and the blind spot named above.
- [`policies/mcp/`](../../policies/mcp) — the shipped rule pack for this substrate, with its scenarios.
- [`docs/LIMITS.md`](../../docs/LIMITS.md) — the effects this project cannot observe from the inside.
