# assets/icon/ — the app mark

> status: authored 2026-08-25 (glovrex/req/820 ATOM 1). Deterministic hand-authored SVG;
> no AI image generation anywhere in this directory's lineage.

## What the mark is, and where every part of it comes from

The mark is the **canon receipt glyph** — the torn-bottom slip — set on the product's own
dark ground. Nothing here was invented for the icon:

| element | source of record |
|---|---|
| slip silhouette + body lines | `research_bus/tracefold_design/mocks_v4/glyph_sheet.html` `#g-receipt`, path data identical |
| 24 grid, stroke 1.7, round caps/joins | same sheet's stated convention ("24 grid, stroke 1.7") |
| tile `#0a0a0c`, strokes `#e8e8e8` | `tokens/tokens.css` `--bg` / `--ink` (dark bucket) |
| no red anywhere | canon rule: "one-point red = deny, nothing else is ever red" — an icon denies nothing |

Why the receipt and not another of the 15 marks: the receipt is the canon's "proof" mark,
and proof is the one thing this product sells. Every other mark names an effect, a verdict
or a party; only the receipt names the product.

## Files

- `icon.svg` — master, 48 viewBox: tile `rx 10.5` + `g-receipt` scaled 2.2x. Use at 48px+.
- `icon-small.svg` — 16/32px raster source only. Same silhouette, simplified where a 3x
  downscale smears the master: 2 tear teeth (not 5), 2 body lines (not 3), stroke 2.2
  (not 1.7). Small-size simplification is icon practice, not a canon fork; `icon.svg`
  stays the shape of record.
- `icon-{16,32,64,256}.png` — rendered set. 16/32 from `icon-small.svg`, 64/256 from
  `icon.svg`.
- `icon.ico` — all four sizes in one container (Windows).

## Reproduction (deterministic, tools on this machine)

PNG set — headless Chrome, transparent ground, one shot per size:

```
chrome --headless=new --disable-gpu --default-background-color=00000000 \
  --window-size=SIZE,SIZE --screenshot=icon-SIZE.png render-SIZE.html
# render-SIZE.html = the svg inlined with css `svg{width:SIZEpx;height:SIZEpx}`, zero margin
# SIZE in {16,32} uses icon-small.svg; {64,256} uses icon.svg
```

`.ico` assembly — ImageMagick, smallest first:

```
magick icon-16.png icon-32.png icon-64.png icon-256.png icon.ico
# verify: magick identify icon.ico  ->  4 frames, 16/32/64/256
```

Web favicon copy: `icon.svg` -> `glovrex/web_site/public/favicon.svg` and `icon-32.png`
-> `favicon-32.png` (placed 2026-08-25; the `<link rel="icon">` wiring into the site head
is the site build's decision, drafted in glovrex `req/820`, not done from here).

## `[ ]` not done

- `[ ]` No `<link rel="icon">` in any shipped page yet — asset placed, head not wired.
- `[ ]` macOS `.icns` — needs `iconutil` (mac-only) or `png2icns`; steps not written until
  a mac build exists to carry it (glovrex `req/820` ATOM 2 marks mac CI-only).
- `[ ]` The canon sheet has no "app icon" row; whether this tile shape joins the sheet is
  a sheet-owning decision, recorded here and not made here.
