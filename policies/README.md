<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# policies/

Cedar policy packs (`.cedar` rule files + their `scenarios.json` conformance fixtures)
that `gx-gate` embeds at build time. Three packs ship publicly: `fs`, `git`, `mcp`.

## Ships to the public repo

- `fs/deny-etc.cedar`, `fs/scenarios.json`
- `git/deny-nonbranch-refs.cedar`, `git/scenarios.json`
- `mcp/deny-etc-resources.cedar`, `mcp/scenarios.json`

Each pack's `scenarios.json` ships beside its `.cedar` file since req/833 — `PACK_FORMAT.md`
F1 asks every shipped pack to co-ship the scenario file it hands a third party, and two
public tests (`pack_locators.rs`, `policy_cmds.rs::every_shipped_pack_passes_its_own_scenario_file`)
hold the tree to that.

## Present here, does not ship

- `policies/postgres/` (`deny-system-catalogs.cedar` + `scenarios.json`) — the private
  `pg`-feature adapter's pack.
- `policies/PACK_FORMAT.md` — the format spec itself.

`tools/pub_sync_dryrun.sh`'s `build_manifest()` names exactly the six files above and
nothing else in this folder; `postgres/` and `PACK_FORMAT.md` exist in this working tree
but do not reach `github.com/TraceFold/tracefold`. This is a selected subset, not the
whole of what `policies/` locally holds.

## Format

What a `policies/<substrate>/` directory has to contain to land in the same shape as the
three above — `PACK_FORMAT.md` (this local tree only, per the split above).

---
Derived from: `tools/pub_sync_dryrun.sh`'s policies loop (fixed 6-file list, cross-checked
against `git ls-files -- policies/`, 9 tracked files total locally), 2026-08-30. req/968 P-968-4.
