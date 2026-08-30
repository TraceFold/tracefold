<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# docs/ (staged here, ships as `docs/README.md`)

Maintainer-side staging copy for the public repo's `docs/`. Per `public/_SYNC_NOTE.md`,
files under `public/` are authored here and never edited in the public repo directly.
`tools/pub_sync_dryrun.sh`'s `build_manifest()` assembles the public `docs/` from two
separate sources:

## Shipped directly from this repo's root `docs/`

- `LIMITS.md`
- `TUTORIAL.md`

(`public/docs/LIMITS.md` and `public/docs/TUTORIAL.md` are maintainer-kept copies of
these same two files, not a third and fourth source — see `public/_SYNC_NOTE.md`.)

## Staged under `public/docs/`

- `DEVELOPMENT_TREE_TESTS.md`
- `ERROR_TAXONOMY.md`
- `RECOVERABILITY.md`
- `TRACEFOLD_TR.md`
- `articles/tamper-evident-audit-trails-compared.md`
- `articles/tamper-evident-receipts.md`
- `articles/verify-ai-agent-actions-offline.md`

This repo's root `docs/` also tracks `ERROR_TAXONOMY.md` locally, but `build_manifest()`'s
root-`docs/` loop only names `LIMITS.md`/`TUTORIAL.md` — `ERROR_TAXONOMY.md` reaches the
public tree through the staged copy above instead, not through that loop.

---
Derived from: `tools/pub_sync_dryrun.sh`'s docs loop, cross-checked against
`git ls-files -- public/docs/` (9 files after de-duplicating the two files staged in both
places), 2026-08-30. req/968 P-968-4.
