<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# assets/ (staged here, ships as `assets/`)

Brand marks, banners, avatars, and article figures used across the public faces
(`public/README.md`, `public/org_profile/*`, `public/user_profile/README.md`) and the
`docs/articles/*` pieces. Staged under `public/assets/` per `public/_SYNC_NOTE.md` —
edited here, never in the public repo directly.

## brand/

- `avatar-512.png`, `avatar-dark-512.png`, `avatar-light-512.png`
- `banner-glovrex.png`, `banner-release.png`, `banner-tracefold.png`
- `hero-glovrex.png`
- `mark.svg`, `mark-dark.svg`
- `social-preview-1280x640.png`
- `tf-banner-src.png`, `tf-square-src.png`

## figures/

- `no-way-back_retention.png`
- `the-counter-was-honest.png`
- `the-ground-moved.png`
- `zero-of-twelve.png`

## marks/

- `carried.svg`, `carried-dark.svg`
- `closed.svg`, `closed-dark.svg`
- `open.svg`, `open-dark.svg`
- `unlooked.svg`, `unlooked-dark.svg`

## top-level

- `glovrex_target_architecture_vision.png`
- `mahirhir_hero_banner.svg`
- `pr_track_record_card.svg`
- `tracefold_architecture_diagram.svg`
- `tracefold_architecture_pipeline.png`
- `tracefold_metrics_grid.svg`

This folder ships to the public repo unfiltered except for `_`-prefixed files (none exist
here today) — `tools/pub_sync_dryrun.sh`'s `build_manifest()` stages everything under
`public/` that `grep -v '/_'` keeps.

---
Derived from: `git ls-files -- public/assets/` (30 files, 2026-08-30). req/968 P-968-4.
