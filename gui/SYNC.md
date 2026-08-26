<!-- SPDX-License-Identifier: Apache-2.0 -->

# How this directory gets here

`gui/` in the public repository is not a second copy of this work that someone edits in
parallel. It is an **image**: a function of one private source tree and one written list of
exclusions. There is exactly one place changes are made, and this directory is what falls out.

Saying it that way is not a style preference. Two editable copies of the same tree is the
shape that produces a public face quietly disagreeing with the thing it claims to describe,
and the whole point of this product is that a claim can be checked against what happened.

## The function

```
image = (git archive <pin>) minus (the exclusions below)
```

Nothing else. No hand edits land in the image, no file is added on the way out, and the
derivation is re-runnable by anyone holding the source at `<pin>`.

## What is excluded, and why

| excluded | why |
|---|---|
| `req/**` | the private planning corpus: internal reqdefs, a competitor teardown, and the bulk of the tree's non-English prose |
| `**/record/critique_*/`, `**/record/req<N>_*/` | design-review capture evidence — screenshots from review cycles, plus the sidecar JSON that describes those screenshots and has nothing left to describe once they are gone |
| other `**/record/*.png` | same class |
| `**/fixtures/shots/measurements.json`, `**/fixtures/shots/browser-mount-smoke.json`, `**/record/shots.json` | machine-written capture output; each embeds the absolute save path of every PNG it wrote |
| `docs/APP_ARCHMAP_*.html`, `docs/ABSORPTION_STATUS.html` | internal status pages, non-English, carrying stale local file:// links |
| `.run/`, `*.log`, `*.bak-*` | scratch. `.run/report.json` declares `citable as evidence: false` about itself |

Nine `record/*.json` files are **not** excluded and must not be: `real-window.json` (×7),
`interaction-pass.json`, `app-window.json` are hand-authored inputs that the real-window
instruments read. Only the captures are evidence. Treating `record/` as uniformly droppable
ships a tree whose own instruments cannot run.

## Checking the correspondence

`node tools/public_image_gate.mjs --pin <sha> --clone <path-to-public-checkout>` rebuilds the
image from the source at `<pin>` and compares it file-by-file against `gui/` in a checkout of
the public repository, by SHA-256. It reports three lists — content differs, only in image,
only in public — and exits non-zero if any is non-empty.

It is a gate, not a sync tool: it never writes to either side. A difference is a finding to be
explained, and the explanation is either "the image was not re-derived after a source change"
or "someone edited the public copy directly", which is the thing this arrangement exists to
catch.

## What this does not cover

Stated because a correspondence check that quietly has holes is worse than none.

- It compares the image against the public tree. It does **not** prove the exclusion list
  above is the right list — that is a judgement, recorded here so it can be argued with.
- It says nothing about the rest of the public repository outside `gui/`.
- A source commit that is never published leaves the public tree simply older; the gate calls
  that a difference, and it is on the reader to know which of the two is stale.
