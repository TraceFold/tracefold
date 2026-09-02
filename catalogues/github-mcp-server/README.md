<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# github-mcp-server

A restore catalogue for [`github/github-mcp-server`](https://github.com/github/github-mcp-server)
(MIT) at tag **v1.9.0**, with the `issues_granular` and `pull_requests_granular` feature flags
enabled. Eleven write tools sit behind those two flags; this pack declares **seven** of them and
names the other four below. Without the flags none of the eleven exists on the server at all, and a
default deployment has four write tools this pack says nothing about.

```
gx wrap --restore-catalogue catalogues/github-mcp-server/v1.9.0.json -- <agent>
```

Where the names come from: `pkg/github/__toolsnaps__` at the v1.9.0 tag, which is upstream's own
JSON snapshot of every tool definition — 116 of them — and therefore the primary source for tool
names and input schemas. The repository README and `docs/` are generated from it and were not used.

---

## What it declares — seven pairs

Each pair is a granular edit undone by re-applying **the same tool** with the value the object held
before. The prior does not come from `resources/read`: v1.9.0 registers five resource templates and
every one of them is a `repo://…/contents…` path, so an issue and a pull request have no read face
at all. Each declaration therefore names a **read tool** (`issue_read` / `pull_request_read`,
`method: "get"`) plus a template that builds the restore call's named arguments out of that answer,
with an identity binding the answer to the object the call was about.

| Forward tool | Restored by | Prior member | Note |
| :--- | :--- | :--- | :--- |
| `update_issue_body` | `update_issue_body` | `/body` | The rendered value is the value the setter takes. |
| `update_issue_title` | `update_issue_title` | `/title` | Likewise. |
| `update_issue_type` | `update_issue_type` | `/type/name` | The setter is `anyOf: [string, null]`, so *removal* is expressible and the round trip is whole in both directions. |
| `update_issue_milestone` | `update_issue_milestone` | `/milestone/number` | Only when the object had one. The setter is `integer, minimum: 1` with no spelling for removal, so a null prior builds **no inverse** — enforced by the pointer failing to resolve, not by a promise. |
| `update_pull_request_body` | `update_pull_request_body` | `/body` | |
| `update_pull_request_title` | `update_pull_request_title` | `/title` | |
| `update_pull_request_state` | `update_pull_request_state` | `/state` | A merged pull request refuses to reopen, **loudly** — `422` from the server. |

## What it refuses to declare, and why

Naming these is half the point of the file. Each has a well-formed-*looking* pair that would be
wrong.

| Tool | Why no pair is declared |
| :--- | :--- |
| `update_issue_labels` | **Projection.** An issue renders `labels` as objects — `[{"id":…,"name":"bug"}]` — and the setter takes strings — `["bug"]`. An RFC 6901 pointer *selects*; it does not project, and no word in the template vocabulary maps a collection of objects onto a collection of their members. |
| `update_issue_assignees` | The same projection gap, and the API additionally drops an un-assignable login **in silence**. |
| `update_issue_state` | The undo succeeds and `state_reason` does not come back: a close carries a reason, a reopen cannot restore one. A **silent partial**, and partial-inverse declarations are not adopted. |
| `update_pull_request_draft_state` | Ready-to-draft clears the requested reviewers and draft-to-ready does not put them back. Silent partial again. |
| `update_pull_request` | Composite, and not flag-gated. Requested reviewers are additive rather than replacing, and a `base` change moves the diff the pull request is about; re-applying the same tool does not close the inverse. |
| `update_pull_request_branch` | The inverse is a force-push of the head to its prior sha, and no force-push tool exists among the server's 116. |
| `create_or_update_file`, `update_gist` | Outside this pack. The first is a contents-face tool whose prior *is* readable through `resources/read`, so it does not need — and is not covered by — the read-by-tool form this pack is built around; the second is not part of the issue and pull-request surface. |

🔴 **The line between the two tables is one criterion.** A boundary that makes the undo fail
**loudly** leaves the pair declarable: the object is either back, or the operator is told it is not.
A boundary that lets the undo **succeed while the object is not back** removes it.
`update_pull_request_state` and `update_issue_state` differ on exactly that and on nothing else.

## 🔴 The gap an operator must read before relying on this pack

**On a real github-mcp-server there is no protection against a concurrent third party.** gx refuses
an undo when the object moved between the escrow and the undo — but that check needs a read face for
the object, and v1.9.0 publishes none for an issue or a pull request. The declared compare-and-set
road cannot stand in: its vocabulary produces a JSON *string* for anything that varies per locator,
and `issue_read` requires `issue_number` to be a **number**, so a per-locator numeric argument has no
word in the shipped vocabulary. The refusal is measured against a face that publishes a `github://`
resource; it is **not** measured against a server shaped like the real one. On the real server, an
edit somebody else makes between the escrow and the undo is overwritten.

Beyond that: a transfer, a deletion, an archived repository or a lost permission makes the restore
call fail with 404 or 403. That is the loud direction and is preferred to a quiet partial restore.

## 🔴 What has not been measured

* **Zero live calls against `api.github.com`.** The schemas were transcribed from upstream's
  toolsnaps at the tag. The **response** shapes — that an issue renders `labels` as objects, and
  `milestone` and `type` as objects — are read off GitHub's published REST schema and are not
  measured. If one of them is wrong, the verdict for that tool moves.
* **The feature flags were never toggled on a running server.** What is established is that the
  eleven tools exist in upstream's snapshots at v1.9.0 and that eleven of the fifteen `update` write
  tools sit behind the two flags.
* **`is_suggestion` is not driven.** A call whose effect the API decides is a third thing this
  vocabulary has no word for, and guessing at it would be worse than leaving it named.
* **No rate limit, no 1 MiB escrow ceiling, no secondary limits.**

What *is* measured is the mechanism. `crates/gx-cli/tests/rmcp1_github_p1.rs` drives this exact
declaration through `gx wrap` over a real wire against a probe process in its github face, with
strict argument validation on, and `crates/gx-adapter-mcp/tests/shipped_catalogues.rs` binds the
bytes shipped here to that declaration. The face is ours, so what those tests prove is that gx does
the right thing with this declaration — not that GitHub answers the way the declaration expects.
