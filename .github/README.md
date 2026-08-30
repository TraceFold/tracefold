<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# .github/ (staged here, ships as `.github/`)

Community/process files for the public repo — contribution guide, code of conduct,
security policy, issue/PR templates. Staged under `public/.github/` per
`public/_SYNC_NOTE.md`.

## Ships to the public repo

- `CONTRIBUTING.md`
- `CODE_OF_CONDUCT.md`
- `SECURITY.md`
- `PULL_REQUEST_TEMPLATE.md`
- `CODEOWNERS`
- `ISSUE_TEMPLATE/1_defect.yaml`
- `ISSUE_TEMPLATE/2_boundary.yaml`
- `ISSUE_TEMPLATE/config.yml`

## Kept here, excluded from the public sync

- `ISSUE_TEMPLATE/_archive/WHY.md`
- `ISSUE_TEMPLATE/_archive/bug_report.md`
- `ISSUE_TEMPLATE/_archive/design_decision.md`

Retired issue-template history. `tools/pub_sync_dryrun.sh`'s `build_manifest()` stages
`public/` through `git ls-files -- public/ | grep -v '/_'`, which drops every path
carrying an `_`-prefixed directory segment; `_archive/` is one, so these three do not
reach `TraceFold/.github` even though they sit under `public/.github/`.

**Note on this folder's own drift gate**: `tools/gates/public_readme_sync_gate.mjs`'s
`deriveGithubFiles()` only excludes basenames that start with `_` — it does not check
directory segments — so it currently treats all 11 files above, including the three
archived ones, as one "shipped" set for its HIT/MISS check. That is looser than
`build_manifest()`'s actual rule. This README states the real (narrower) split above
rather than the gate's approximation, and still lists all 11 basenames so the gate's
table-of-contents check finds them.

---
Derived from: `git ls-files -- public/.github/` (11 files, 2026-08-30), cross-checked
against `tools/pub_sync_dryrun.sh`'s `grep -v '/_'` staging rule. req/968 P-968-4.
