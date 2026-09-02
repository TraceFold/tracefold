<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# catalogues/

Restore catalogues. A catalogue is a deployment's declaration of **which tool undoes which**, for
one MCP server, and `gx wrap --restore-catalogue <FILE>` is the one road it reaches a session by.
[`gx-adapter-mcp`](../crates/gx-adapter-mcp)'s `Catalogue::from_json` is the format's one reader.

---

| Pack | Declaration | Server it is written against | What it declares |
| :--- | :--- | :--- | :--- |
| **github-mcp-server** | [`github-mcp-server/v1.9.0.json`](github-mcp-server/v1.9.0.json) | `github/github-mcp-server` v1.9.0, with the `issues_granular` and `pull_requests_granular` feature flags on | Seven granular issue and pull-request edits, each undone by re-applying the same tool with the prior value read back. [Its README](github-mcp-server/README.md) names the tools it refuses to declare, and why. |
| **notion-mcp-server** | [`notion-mcp-server/2.5.1.json`](notion-mcp-server/2.5.1.json) | `makenotion/notion-mcp-server` at commit `1d38420` (package `2.5.1`, untagged), `Notion-Version: 2025-09-03` | One pair — page create undone by deleting the block a page is — whose restore argument is read from the *forward call's own result*, not a prior read. [Its README](notion-mcp-server/README.md) names the operator-facing gap between this pack's local-server evidence and Notion's now-recommended remote server. |

This is the same shape [`policies/`](../policies) has: an artefact an operator would otherwise
write by hand, shipped beside the check that proves it says what it claims. Here the check is
[`crates/gx-adapter-mcp/tests/shipped_catalogues.rs`](../crates/gx-adapter-mcp/tests/shipped_catalogues.rs),
which reads every file in this tree through the shipped reader and fails on a pack it does not
name — so a pack cannot arrive here without a check arriving with it.

---

## What a catalogue does and does not decide

A catalogue is **not a second gate**. It does not decide which tools may be called: a tool no
catalogue has heard of is planned, carried and — if a policy admits it — called, exactly like one
a catalogue knows. What it changes is one question, *can this be undone?*, and the answer reaches
the gate as `invert_available`. An undeclared tool is therefore one whose calls are irreversible as
far as gx knows, and a change with no inverse is escalated to a person rather than admitted
silently. **An empty catalogue is the conservative direction, not a broken one.**

A catalogue does not make an undo *correct*. It says a deployment claims this call undoes that one;
whether the claim holds is the deployment's responsibility, the same way a policy encoding the
wrong intent is enforced faithfully. See [`docs/LIMITS.md`](../docs/LIMITS.md).

## The `$server` pin is carried, not verified

Every pack in this tree names, in its `$server` slot, the server and version its declarations were
written against — because a catalogue is a claim about a *specific* server's tools, and the same
tool name on a different server can mean something else. **gx does not check the pin against the
live server.** Nothing in the wrap path compares `$server` to what the server answers at handshake,
so pointing a pack at the wrong server is a mistake the operator makes and gx does not catch. This
is a named absence rather than an oversight discovered here: `crates/gx-adapter-mcp/src/catalogue.rs`
says it where the slot is defined, and records the `tools/list` cross-check as `req/182` M-25's
unspent candidate.

## Adding a pack

A pack is a directory holding a `README.md` and one version-named `.json` declaration. Both are
required, and the check file above must be extended to name the new pack — a pack it does not name
fails the tree enumeration. The README carries three things the JSON cannot: where the tool names
were read from (upstream's own machine-readable source, not its generated docs), which tools the
pack **refuses** to declare and why, and which of the declared pairs stop being reversible under a
named condition.
