<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# tools/

One file out of the 236 tracked here ships to the public repo: `e2e.sh`.

## Ships to the public repo

- `e2e.sh` — clones this repository from git and builds + tests only what the clone
  holds, so a green working tree can never be mistaken for a green repository (script's
  own header; `req/05` §3/§4). An anonymous clone of `TraceFold/tracefold` has been
  measured to carry exactly this one file under `tools/` (req/817).

## Does not ship

Everything else — lane scripts, audit probes, `gates/` (15 files), one-off `m6h*`/
`m6fix*` build helpers, and `pub_sync_dryrun.sh` itself — is private build/lane
scaffolding. `pub_sync_dryrun.sh` in particular must never ship: it carries the
leak-detector patterns (including retired handles) it searches the tree for, and shipping
it would ship the search terms alongside whatever they are meant to catch (req/789 §4).

`tools/pub_sync_dryrun.sh`'s `build_manifest()` declares `e2e.sh` as the sole member of
this folder headed for the public tree.

---
Derived from: `tools/pub_sync_dryrun.sh`'s tools loop, cross-checked against
`git ls-files -- tools/` (236 tracked files, 15 under `gates/`), 2026-08-30. req/968 P-968-4.
