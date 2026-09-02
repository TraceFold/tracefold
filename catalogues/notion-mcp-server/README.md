<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# notion-mcp-server

A restore catalogue for [`makenotion/notion-mcp-server`](https://github.com/makenotion/notion-mcp-server)
(MIT, Copyright Notion Labs, Inc.) at commit **`1d38420`** (`package.json` version **2.5.1**), against
the Notion API pinned to `Notion-Version: 2025-09-03`. One write tool is declared; the other roughly
twenty on this server are not — not because each was found broken, but because only this one round trip
has been driven against the real server (`req/183`).

```
gx wrap --restore-catalogue catalogues/notion-mcp-server/2.5.1.json -- <agent>
```

Where the name comes from: `scripts/notion-openapi.json` at the pinned commit, upstream's own
machine-readable OpenAPI spec — the same document the server's tool handlers are generated from. The
repository README's operation names (`post-page`, `delete-a-block`, …) are the spec's `operationId`s;
the live `tools/list` answer prefixes each with `API-` (`API-post-page`, `API-delete-a-block`), which is
the form this pack declares in and the generated docs do not show. The same gap `catalogues/
github-mcp-server/README.md` calls out for GitHub's toolsnaps.

**No git tag names this commit.** The latest tag is `v2.5.0`; `1d38420` is `main`'s tip and carries the
version bump to `2.5.1` in `package.json` with no corresponding tag (verified against the live
repository on 2026-09-02 — `main` has not moved since `req/183` measured it on 2026-08-15). The pack is
pinned to the commit for that reason, and the version file is named after the untagged package version
rather than a release that does not exist.

---

## What it declares — one pair

| Forward tool | Restored by | Prior member | Note |
| :--- | :--- | :--- | :--- |
| `API-post-page` | `API-delete-a-block` | none — the inverse's one argument is the object's own id, read back from the *forward call's own result* | A page is a block; `DELETE /v1/blocks/{id}` sets `in_trash`/`archived` true. Measured against the real server on 2026-08-15 (`fixtures/notion-post-page-observation.json`). |

The restore call's one argument, `block_id`, is built from `{"do_result": "/id"}` — the created page's
own UUID, which the server's create-time answer carries verbatim. No escrow-time material and no prior
read are involved: the object does not exist until the forward call returns, so there is nothing to read
beforehand (`crates/gx-adapter-mcp/tests/notion_page_catalogue.rs`'s module doc). This is unlike every
pair in the GitHub pack, which all read a prior *before* the forward call — Notion's declared pair is
the one shape in this tree that needs no read face at all.

## Why the compensating tool is a `DELETE`, not the setter the API reference suggests

The reference document's own worked example for undoing a page create is `PATCH /v1/pages/{id}` with
`in_trash: true` (`req/169` §8). As a **tool argument** that value is a JSON boolean, and this format's
constant word (`ArgSource::Const`) only ever produces a JSON *string* — `{"const": "true"}` sends
`"in_trash": "true"`, which the Notion API's strict body validation refuses (measured,
`crates/gx-adapter-mcp/tests/notion_page_catalogue.rs::the_constant_word_is_string_only_which_is_why_
the_pair_avoids_a_boolean_argument`). `API-delete-a-block` reaches the same server state — the page
(itself a block) trashed — with the one argument the vocabulary can already supply, so the pair is
declarable today with no vocabulary change. A typed-constant word that could carry a real boolean is
open as `DR-V4B-2`, not quietly added here.

## What it refuses to declare, and why

| Tool | Why no pair is declared |
| :--- | :--- |
| `API-patch-page` | **Measured, not merely unattempted.** The natural restore for a property edit (`in_trash`, `archived`, or any other member) takes a JSON body the constant word cannot spell without lying about its type — the same gap the paragraph above measured for the create/trash pair. |
| Every other write tool (`API-post-page` with a full property body rather than the minimal one this pack targets, `API-update-a-block`, `API-patch-block-children`, `API-move-page`, `API-create-a-comment`, `API-create-a-data-source`, `API-update-a-data-source`, and the rest of the ~20 write-capable operations the spec lists) | **Not measured.** `req/183` drove exactly one round trip against the real server — page create, undone by block delete — and no other tool has a captured request/response pair to declare a template against. An undeclared tool is not a claim that it is irreversible; it is the conservative default this pack has not tried to move (`catalogues/README.md`). A property-update pair (editing one member of an existing page and restoring it from the prior read) needs a template word this format does not have yet — extracting one member's prior value out of a whole-object JSON read — tracked as future vocabulary work in `req/169` §8(ii), not attempted here. |

## 🔴 The gap an operator must read before relying on this pack

**This pack was written against the *local, self-hosted* `notion-mcp-server` process** (`stdio`,
launched with an integration token) — the server the pinned commit and OpenAPI spec describe. Notion's
own README, as of the pinned commit, says the company is "prioritizing, and only providing active
support for, **Notion MCP** (remote)" — a separately hosted server at `https://mcp.notion.com/mcp` —
and that "issues and pull requests here are not actively monitored" and the local repository "may [be]
sunset ... in the future." Whether the remote server's tool names, argument shapes, and result shapes
match this pack's is **not verified by this pack**: `crates/gx-adapter-mcp/tests/notion_page_catalogue.rs`
carries a second, separate test arm (`a_cas_read_declaration_gives_the_tools_only_page_a_compare_and_set`)
against a stand-in for the shape the remote server is measured to have — tools-only, no `resources/read`
face at all — using the **same** tool names and the **same** declared pair, but that arm is a fixture
written from the local server's schema, not a live call to the remote endpoint. `catalogues/README.md`'s
"the `$server` pin is carried, not verified" applies here with an extra edge: the pin names one process
among (at least) two Notion ships, and gx does not check which one answered.

Beyond that: a page under a workspace the integration cannot see, a revoked integration, or a page
already in the trash makes the restore call fail with 404 or a validation error rather than silently
doing nothing — the loud direction, and preferred to a quiet partial restore.

## 🔴 What has not been measured

* **`gx undo` has not gone green against a real server through `gx wrap`.** `req/183` §0 states this
  plainly: the *declaration* is measured (this pack, `notion_page_catalogue.rs`'s 8 tests), and the
  *server round trip* was measured separately and manually (`tools/a2b_notion_raw_pair.py`, raw protocol,
  no gx in the loop) because `gx wrap`'s plan step refuses first — `notion-mcp-server` declares no
  `resources` capability (`initialize` → `{"tools":{}}`), so the escrow road's unconditional prior read
  has nothing to read from for this pair, and `snapshot` fails before `invert` is reached. This is a
  structural gap tracked as `DR-V4B-1`, open at the time this pack ships.
* **The remote server** (`https://mcp.notion.com/mcp`), which Notion's own documentation now directs
  new integrations toward, has not been queried by this pack's evidence at all — see the gap above.
* **Rate limits** (Notion's documented ceiling is roughly 3 requests/second) are not measured against
  this pack; `req/183` §5 reports zero observed `429`s across one lane's call volume, which is not a
  measurement of the limit.
* **A second write** landing between escrow and undo is not guarded for this pair the way the GitHub
  pack's compare-and-set is: this declaration's template draws only on the forward call's own result
  (`do_result`), not on a prior read, so there is no fingerprint to compare against at undo time for the
  *declared* pair. The tools-only `$cas_read` arm (`fixtures/notion-page-catalogue-cas.json`, not shipped
  here) gives a compare-and-set for *reading* the page, but `crate::invert`'s unconditional prior read
  still blocks the escrow half on a resource-less server, so the two roads do not yet compose for this
  pair — held as a fact in the module doc's own words, not fixed by this pack.

What *is* measured is the mechanism, on the local server's shape: `crates/gx-adapter-mcp/tests/
notion_page_catalogue.rs` drives this exact declaration through the escrow, completes the inverse from
a captured real server response, and folds to `None` (no undo minted) on every malformed or error
observation it was given. `crates/gx-adapter-mcp/tests/shipped_catalogues.rs` binds the bytes shipped
here to that declaration.
