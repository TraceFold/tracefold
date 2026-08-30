<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# assets/

Every image this repository's pages reference. Listed rather than shown: a directory that renders
thirty images is slower to load than the pages that use them, and tells you less.

---

## brand/ — marks, avatars and banners

| File | Used for |
| :--- | :--- |
| `mark.svg`, `mark-dark.svg` | The project mark, light and dark variants. |
| `avatar-512.png`, `avatar-light-512.png`, `avatar-dark-512.png` | Square avatars at 512px. |
| `banner-tracefold.png`, `banner-glovrex.png`, `banner-release.png` | Wide banners for the repository, the organisation, and release pages. |
| `hero-glovrex.png` | Organisation hero image. |
| `social-preview-1280x640.png` | The link-preview card at the size social platforms crop to. |
| `tf-banner-src.png`, `tf-square-src.png` | Uncropped sources the banner and square variants are cut from. |

## marks/ — the four verdict marks

| File | Meaning |
| :--- | :--- |
| `open.svg`, `open-dark.svg` | A change that is still open. |
| `closed.svg`, `closed-dark.svg` | A change that was closed. |
| `carried.svg`, `carried-dark.svg` | A change carried forward. |
| `unlooked.svg`, `unlooked-dark.svg` | Something not examined — distinct from something examined and found clean. |

Each mark has a `-dark` variant because a single image tinted for one background reads as damage on
the other. The "unlooked" mark exists for the same reason the product keeps a third verdict value:
not-checked and checked-and-clean are different facts and must not share a picture.

## figures/ — article figures

`no-way-back_retention.png`, `the-counter-was-honest.png`, `the-ground-moved.png`,
`zero-of-twelve.png` — figures belonging to the pieces under [`docs/articles/`](../docs/articles).

## Diagrams and cards

| File | Used for |
| :--- | :--- |
| `tracefold_architecture_diagram.svg`, `tracefold_architecture_pipeline.png` | The gate-and-receipt pipeline, as a diagram and as a rendered strip. |
| `glovrex_target_architecture_vision.png` | The wider architecture this project implements two layers of. |
| `tracefold_metrics_grid.svg` | The measured-status grid. |
| `mahirhir_hero_banner.svg`, `pr_track_record_card.svg` | Maintainer profile artwork. |
| `README.md` | This page. |

---

## Referencing these files

GitHub does not serve this directory's raw bytes reliably to other pages — a `raw` URL can be rate
limited and a `/raw/` path can 404 — so pages that must render an image point at a release asset
instead of at this folder. These files are the sources those assets are cut from; edit here, then
re-publish the asset.
