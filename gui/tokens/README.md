# tokens/ — the one physical stylesheet of record, owned in-repo

> **status**: landed 2026-08-24 (req/03 SS635, glovrex/req/38 -- the F-3 receipt lane's
> tokens-self-ownership atom). `tokens.css` replaces a pointer into
> `TraceFold_App/ui_proto/ui/tokens.css` that violated the SS24 retirement methodology: a
> live app dependency on the reference tree this whole project exists to supersede.

## Why this directory and not somewhere inside `parts/`, `shell/kernel/`, or a face

Every one of those trees already carries its own "no colour literal here" gate, each
scoped to its own directory:

- `parts/test/tokens.test.mjs`'s "there is no second roster" test walks the whole
  `parts/` package (excluding `shots/` and `generated/`) for any `.css`/`.mjs`/`.js`/
  `.html` file that declares `--bg`/`--ink`/`--line`/`--deny`/`--row`/`--pad-x`/
  `--spine-x`.
- `shell/tools/gates.mjs`'s "the frame writes no colour" gate scans `shippedFiles()`
  (`shell/kernel/**` + `shell/demo/**`, excluding `.gen.*`) for hex literals and colour
  functions.
- Every `faces/*/tools/gate.mjs` runs the equivalent check over its own `FACE_ROOT`.

A real token declaration placed inside any of those trees would trip the gate built to
catch exactly that. This directory sits outside all of them on purpose, so the one
place a colour is written is not mistaken for a second roster smuggled into a package
that is supposed to hold none.

## Consumers

- `parts/tools/token-source.mjs` (`TOKEN_SOURCE_RELATIVE`) -- the Node-side resolver
  every build-time/test-time reader goes through.
- `parts/tools/generate-tokens.mjs` -- reads this file once at build time, writes
  `parts/generated/tokens.generated.mjs` (the mirror a browser-loaded module reads,
  since a real browser has no `node:fs`), stamped with a SHA-256 of these exact bytes.
- `shell/tools/serve.mjs` (`TOKENS_DEFAULT`) -- answers `/s-common/tokens.css` from
  this file directly (not a copy) for every real-window/browser-mount capture.

## What was derived, and from what

Every colour and size in `tokens.css` was computed from the written design canon
(`glovrex/req/38` SS551 Docker-IA chrome, SS553 36px tap targets, SS558 14px body
floor) against the WCAG relative-luminance formula, not read or copied from the
retired file's own hex values -- see `tokens.css`'s own header for the full account,
including the one item this atom could not close (light-default vs. "do not nail the
theme to the page" -- filed already in `parts/README.md`, needs a shell-level ruling,
out of this atom's scope).

## A second defect, found while verifying the first fix

Repointing both readers at `tokens.css` was not enough on its own: `document.styleSheets[0].cssRules.length` still came back `0` in every renderer (headless and real-window). The cause was in this file's own header comment, which originally contained the prose "faces/*/ tree" -- and `*/` is also the CSS comment-close token, so the very first comment silently truncated the whole stylesheet to nothing (Chrome does not error on this; it just drops everything after the accidental close). Fixed by rewording the prose so no unintended `*/` sequence exists outside an intended close -- verified with a standalone comment-stripping simulation and a real-renderer `document.styleSheets`/`getComputedStyle` check before trusting the fix. Every capture taken after this fix (`faces/receipt/`'s fixtures, browser-mount-smoke, and real-window pair) shows the actual derived palette; this is the first face in the app tree for which that is true.

## Five-principles checklist (`INHERITED_PRINCIPLES.md §3c`)

1. **template-form** -- N/A as "template" in the row-grammar sense, but the file follows the one architectural template every stylesheet-of-record consumer in this tree already assumes (bare `:root` = dark bucket, `--l-` prefix = light bucket, `[data-theme]` override, `prefers-color-scheme` fallback, `--detail-x` collapse) -- not a second shape invented for this atom.
2. **lightweight+bench** -- PASS with a figure, indirectly: this atom adds no runtime code (a `.css` file plus two constant-string edits), so there is nothing of this atom's own to bench; the parts/faces that consume it already carry their own bench figures (`faces/receipt/README.md`: median 0.579ms; `faces/held/README.md`: median 104.05ms; `faces/notice`: 4.20ms), unaffected by this atom (no new computation was added to any hot path).
3. **english+comments** -- PASS. `tokens.css`'s header and every declaration comment are English; the comment-close defect above is documented in the same file, not silently fixed.
4. **always-CRUD** -- N/A. This is a static asset (a stylesheet), not a resource with lifecycle acts.
5. **DB-principle** -- PASS. This file is the one place a colour is declared (C1's own contract, `parts/src/tokens.mjs`); nothing consumes it and also caches or re-derives a colour value -- `parts/generated/tokens.generated.mjs` is a build-time mirror with its own drift gate (`tokens-generated.test.mjs`), not a second source of truth.

## `[ ]` not done

- `[ ]` No semantics-registry row (`req/99`): its census command (`find membrane shell
  parts faces tools -type f`) does not walk this directory. Extending that census is a
  `req/99`-owning decision, not made here.
- `[ ]` `parts/test/tokens.test.mjs`'s "the stylesheet of record is where this package
  says it is" test still fails on one sub-assertion (`tokenSourceRealPath().endsWith(
  'ui_proto/ui/tokens.css')`) -- a retired-tree-specific literal that contradicts
  from-zero ownership by construction (there is no honest way to place an in-repo,
  self-owned file at a path ending in `ui_proto/ui/tokens.css`). Left red and reported
  rather than silently rewritten (SS234 rule) -- see the F-3 lane's own report for the
  full account. The other two previously-failing tests (`tokens-generated.test.mjs`,
  both digest/byte-identity checks) are green.
