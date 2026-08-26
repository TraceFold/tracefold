# faces/notice -- **what this window said, about itself** (F-5)

> **the numbers in the status line below are the 2026-08-24 lane's -- superseded, see
> "Retrofit r4" at the end of this file for the current ones (73 tests, 14 gate checks,
> 7 declared omissions).** The line is kept rather than edited, so what was true then is
> still legible.
>
> **status**: implementation landed 2026-08-24. `node --test "test/*.test.mjs"` = **47 pass / 0 fail / exit 0**. Real-renderer captures = **7** (3 pages x narrow/wide/dark, wide only for the representative page), every reading clean. `membrane/test/discipline.test.mjs` (D1-D6, app-wide D5) = **17 pass / 0 fail / exit 0** with this face's four shipped files in the scanned set.
> **origin**: written from `req/03_FACES_REBUILD.md` §2/§3-1/§5 (the `notice` row) and `req/01_MEMBRANE.md`. **COPY HARD BAN kept structurally**: the retired tree's `faces/notice/*` was never opened, and `faces/ledger/*` in this same repository was never opened either -- not the module, not its declaration, not its tests, not its tools. Every file here was authored fresh; only the *shape* (declaration / binding / face-module / index / `tools/gate.mjs` / `tools/fixture.mjs` / `tools/shoot.mjs`) follows the pattern `faces/ledger` already proved, per the dispatching lane's explicit instruction. **External OSS consulted: none.**

## Finishing lane, 2026-08-24 (req/38 SS551/SS558/SS576, app req/97 Pass 2)

**SS558 body-text floor**: every `T.meta` (12px) body-text call site now reads `T.record`
(14px) -- `aside`/`plain`/legend/omitted-lines/tally, a token swap only. The one exception,
the `at` timestamp column in `entryBlock`/`abstractedBlock` (`T.time`, 13px, mono), is a
documented one: that column is a fixed 72px cell, and bumping its font past what
`req/100_PLACEMENT_SPEC.md` measured it for reproduced req/03's own N-4 defect (a value
clipped on the line with no full copy elsewhere) on first attempt this lane -- caught by
`tools/shoot.mjs`'s `clippedWithoutFull` reading going from 0 to 2, fixed by adding
`overflow-wrap: anywhere` to the one span that needed it (the outcome cell, not the time
cell) rather than reverting the font size. Full sequence, kept here rather than silently
folded away: bump landed → `clippedWithoutFull=2` → root-caused to one unbroken word
("partially_answered,") no longer fitting its `minmax(0,7rem)` track at 14px → fixed →
re-verified `clippedWithoutFull=0` on all 7 captures, both themes.

**Abstraction duty (SS558 §24i②, coordinator-relayed)**: near-duplicate entries now
collapse to one row with a count and a drill-down, rather than drawing one row per array
position regardless. `groupRuns()` groups **consecutive** entries whose
(`through`,`outcome`,`code`,`status`) match exactly and whose address matches with a
trailing run of digits stripped (`get_transformations_0` .. `_41` all group; a genuinely
different route never does, and grouping is never non-consecutive -- two identical entries
with a different one between them stay two rows, two different moments). The shipped
`notice-overflow` fixture (200 entries, differing only by an embedded counter) now draws
as **1 visible row + a closed drill-down**, not 200 -- `tools/shoot.mjs`'s `entries` reading
(a census of `[data-entry]` ids, not visible rows) still reports **200**, because every
individual record keeps its own `data-entry` inside the drill-down: abstraction changes how
many rows a run of identical facts draws, not how many facts are still individually
reachable and counted. Visible text on that fixture fell to 2107 characters total (was the
literal RC-2 defect: 42 rows, ~92% byte-identical, no group even possible before this).
Every entry's primary fact (`entryAddress`, "what was called") now draws bold (`lead()`, a
new helper alongside `plain`/`aside`) rather than at the same weight as the detail/wire-code
lines under it -- explicit hierarchy, costing no column width because the address sits in
the entry grid's flexible `minmax(0,1fr)` cell, not a fixed one.

**Verified**: `node --test` 47/47 (12 new assertions were not needed here -- the existing
suite already covers entry rendering per-record and the grouping is exercised by the same
fixtures those tests already build). `node tools/fixture.mjs && node tools/shoot.mjs`:
`repeated=0`/`clippedWithoutFull=0`/`overlaps=0` on all 7 captures, both themes.
`req/97` Pass 2 re-scored this surface: **verdict 0 → 3** (0 was RC-2 itself; this lane's
grouping is the structural fix that makes RC-2's shape impossible to recur, not merely a
one-off content edit). Worst defect named there: RC-5 (no chain from this face to the
ledger/held faces) is still open -- this lane's scope was the two relayed repair rows only.

## Re-skin lane, 2026-08-24 (req/97 RC-1/RC-2 repair)

**What changed**: the 42-near-identical-row problem (req/97 RC-2: `notice-overflow`, every
row repeating "through the shell"/"asked, not yet answered" verbatim) fixed by moving the
per-entry render from one comma-joined sentence into a compact grid row (time | via-glyph |
address | outcome) and stating what a via-glyph / outcome word / wire code means exactly once,
in a collapsed `legend` `<details>`. Layer vocabulary ("through the shell", "through the
membrane") removed from all visible text; replaced with `effect/network` and `effect/message`
glyphs (own coined marks -- `parts/src/glyph-sheet.mjs`), the raw layer word kept only in the
glyph's own accessibility label, never printed. `gx_code` wire tokens (e.g. `IDEMPOTENCY_
CONFLICT`) stay on screen but on their own labelled `wire code:` line instead of folded into
running prose, so the source of a token is never ambiguous. The H1 banner and always-open
"why" paragraph removed/folded per Owner #284 SS549; the `real_window` mark regression (same
root cause and same fix as `faces/ledger`, see that README) is fixed here too.

**5-principles self-assessment**: 1 template-form PASS (composition-A distinct-data-per-row
grammar, same evolution pattern as ledger); 2 lightweight+bench PARTIAL (no dedicated bench
harness of this lane's own; `faces/notice/tools/bench.mjs`, written by a concurrent 5-
principles lane, is not this lane's figure to claim); 3 english+comments PASS; 4 always-CRUD
PARTIAL by design (C-7: this face reads nothing and writes nothing, `CONSUMES` is frozen
empty -- there is no CRUD surface to expose because the face's entire contract is "draw the
window's own record of what it asked," never "ask" or "act"); 5 DB-principle PASS (every
entry is rebuilt fresh from the window's array on each paint, per the face's own stated
invariant; nothing here is a second copy of anything).

**What was explicitly not done**: same disposition as `faces/ledger`'s README -- the Docker-IA
shell chrome (SS551) is shell-layer scope and was not built here; the from-scratch rewrite
authorized by SS555 was declined in favour of the verified incremental path (see that README
for the full reasoning, shared verbatim across both faces).

## Naming note

the dispatching brief called this lane "F-2". `req/03_FACES_REBUILD.md` §2's own table numbers `held` as F-2 and `notice` as F-5. The brief's scope description (C-7, `consumes: []`, "escalation/notice presentation... not in ledger", the wire-touchless face) is unambiguous and matches only `notice`/F-5 -- `held` (F-2) declares six methods and answers a different question ("what has not happened yet"). This lane built the face the scope actually describes; the label mismatch is recorded here rather than silently resolved.

```
node --test "test/*.test.mjs"     # 47
node tools/fixture.mjs            # writes fixtures/*.html
node tools/shoot.mjs              # real renderer: captures + readings
```

**`record/real_window_*.png` (2026-08-24 repair lane declaration, app req/98 V-13):** regenerate with `powershell -File ../../shell/tools/real_window.ps1` against this face's mounted window. Not reproducible pixel-for-pixel -- live `CopyFromScreen` captures of a real OS window, same as `faces/ledger`'s equivalent note.

Zero dependencies, node builtins only. No `package.json`.

## The shape

| file | what it is |
|---|---|
| `declaration.mjs` | `consumes: []` (C-7) -- the whole of what this face declares to send; marks / order / undrawn / tests |
| `binding.mjs` | the seam to the parts. Only `element`, `tokens`, `glyph`/`sheet`, and `row-order` are drawn from -- no receipt row, badge, fold, or seal, because this face draws no checkable claim |
| `notice.mjs` | `createFace({parts})` -> `{ mount, read, view, toRecord }`. `mount(host, port, notices, {pollMs}) -> unmount`. The fourth parameter carries a default, so `mount.length === 3` and the shell's call site is unaffected |
| `tools/gate.mjs` | 14 machine checks: 10 over shipped source, 3 over what was drawn, 1 over the disk |
| `index.mjs` | the one door a shell mounts |
| `tools/fixture.mjs` | three states written out as pages a browser can draw |
| `tools/shoot.mjs` | the same pages in front of a real renderer, photographed and measured |

## What this face is, and is not

This is the face `req/03` C-7 asks for: at least one face that reaches no method of the server. It reads nothing from a route; the whole of its input is the `notices` array the shell hands every face as its third mount argument -- the window's own record of what it asked and what came back, written by `membrane/src/membrane.mjs`'s `note()` and `shell/kernel/shell.mjs`'s `watch()`/`act()`. This face draws that record. It offers no act, verifies nothing, and names no actor.

**Live growth is polled, not pushed.** `notices` is a plain array the shell and the membrane both mutate by reference; nothing tells this face when it grows. `mount` starts an interval (`POLL_MS = 400`ms default, overridable per the note above) that repaints only when `notices.length` has actually changed, and `unmount` clears it. This was a design decision made in this lane, not dictated by `req/03`: a face built to answer "what did this window say" that only ever showed the state at the instant it was mounted would go silent the moment anything happened afterward, which is the one failure this whole application exists to refuse. `unmount.repaint()` exposes the same check for a caller (or a test) that wants it forced rather than waited for.

**A budget, stated rather than hidden.** Past `DISPLAY_CAP` (200) entries, rows stop being drawn one by one; the count still arriving is stated instead, and the rows already on screen are left in place rather than pushed out to make room -- so a reader partway down the screen never finds the top of it has moved.

## The contracts (`req/03 §3-2`)

| # | how it is held | state |
|---|---|---|
| C-1 | `consumes = []`; there is no caller to guard, because there is nothing this face is permitted to call | [●] vacuously -- the source gate's `no-method-literals-outside-the-declaration` and `no-network` checks confirm no method name or network call exists anywhere in the shipped source |
| C-2 | `sends = []`, `withheld = []` -- a face that declares nothing has nothing to split | [●] |
| C-3 | six declared omissions with reasons (`declaration.mjs` `UNDRAWN`), plus the entries-past-budget line stated on screen every time it applies | [◐] |
| C-4 | every `data-mark` drawn is in the declared set (`structure/hole`, `undefined`); the check refuses to pass on an empty population | [◐] |
| C-5 | one meaning (`structure.hole`, `mark.undefined`) never carries two marks, checked over the drawn tree | [◐] |
| C-6 | `order.position = 5` + reason, `rows.order = 'as-recorded'` + reason, both stated on screen | [◐] |
| C-7 | **this face** -- `consumes.length === 0`, confirmed both by source scan and by a runtime test that mounts against a port which throws on any property access | [●] |
| C-8 | the declaration names its three tests and the gate checks each is on disk | [◐] |

`[●]` above means this lane's own measurement, not an independent re-run -- see "not independently re-verified" below.

## The negative truth (`req/03 §4`)

| # | how it is held | measured | state |
|---|---|---|---|
| AC-F1 | nothing in this face is positioned; every note is an ordinary block (`overflow-wrap: anywhere`, never a fixed-pitch cell, so there is no clip-and-repeat-in-full mechanism to get wrong the way `faces/ledger` did) | `overlaps=0` on all 7 captures, looked at | [◐] |
| AC-F2 | one entry, one `entryBlock` call, one row per array position | `repeatedEntries=0`, `sprites=1` on all 7 | [◐] |
| AC-F3 | size is a required argument of `glyph` (parts-level guarantee, unchanged); this face never calls it without one | `oversizeGlyphs=0`, `filledGlyphs=0` on all 7; 2 glyphs on the representative page | [◐] |
| AC-F4 | the captures were opened and read by eye | **4 of 7** viewed in full: `notice_narrow`, `notice_dark`, `notice-overflow_narrow`, `notice-empty_narrow` | [◐] |
| AC-F5 | this face's addition to the negative-truth ledger, below | -- | [◐] |

### Negative truth ledger, this face's addition

| # | must not recur | found | AC |
|---|---|---|---|
| N-5 | a live record that stops updating the instant its window is mounted, because nothing tells the face its own array grew | this face's initial design draft (before the poll was added) | AC-F4's general form: a screen that goes quiet without saying so is the product's worst failure, and a face whose entire subject is "what happened" going stale on its own screen is that failure with no route call involved to blame it on |

This is not one of the N-1/N-2 pixel-level defects `req/03 §4` names -- it is a functional-staleness failure this lane found in its own draft, not in the reference tree, and is recorded here because `req/03` AC-F5 asks that a face's own new finding be appended, not only relayed ones. `N-3`/`N-4` remain `req/03`'s own rows; this lane did not touch that file per its instruction.

## Every gate has gone red at least once

Ten source gates (`no-network`, `no-foreign-import`, `no-verification`, `no-actor-named`, `no-colour-literal`, `no-borrowed-symbol`, `nothing-out-of-flow`, `no-method-literals-outside-the-declaration`, `no-dynamic-code`, `entries-are-not-edited`) each have a planted string in `test/gate.test.mjs` that makes them red. So do the three drawn-tree gates and the on-disk one.

## `[ ]` -- not done, and not called done

- `[ ]` **No real-window mount.** Exercised only against `test/dom-stand-in.mjs`. `parts/src/tokens.mjs` reads its roster from a build-time-generated module now (no `node:fs` at import time), so the drawing code should in fact load in a browser with no build step -- but this lane did not build or run `browser-mount-smoke.mjs`/`real-window-smoke.mjs` the way `faces/ledger` did. `req/02` W15 is open here for the same reason it is open there.
- `[ ]` **Independent re-run `[●]` = 0.** Every number in this file is this lane's own. `req/05_REFERENCE_TRUTH.md` was not consulted (per `req/03 §3-3`, it carries no per-face PASS count this face could be measured against anyway).
- `[ ]` **Not registered with the harness.** `tools/faces.json` was not touched; this lane's write scope was `faces/`.
- `[◐]` **Entry-render bench added (2026-08-24 repair lane, app req/98 V-1).** `node tools/bench.mjs` times `face.view(state)` over 1,000 entries (4 outcome shapes), median of 5, persisted to `.bench/report.json`, hard-red budget (300ms). **Mount time is still unmeasured**, same gap `faces/ledger` reports.
- `[ ]` **The poll interval was chosen, not derived.** `POLL_MS = 400` and `DISPLAY_CAP = 200` are this lane's judgment calls, stated as such in `notice.mjs`'s own comments, not values read off a spec or a measurement.
- `[ ]` **`unmount.repaint` is not part of the mount contract.** It is a convenience this face adds; a shell that only ever calls the three-argument form never sees it, and no test outside this face's own suite exercises it.
- `[ ]` **The exception-shaped entry (`"exceptions the shell caught" -- req/03 §5`) is untested against a real one**, because nothing in this codebase writes one into `notices` yet. `UNDRAWN` documents the generic rendering path that would draw it once something does; this is a forward-looking design note, not a verified connection.
- `[ ]` **3 of 7 captures** (`notice_wide`, `notice-empty_dark`, `notice-overflow_dark`) were measured but not opened by eye in this session.

## Retrofit r4, 2026-08-25 (Owner #340: not monotone / understandable at a glance / usable)

**A band before a word.** Five figures at the head of the face -- `calls` (everything the
window wrote down), `answered`, `refused`, `absent` (carrying the hole mark), `repeats`
(runs the abstraction collapsed) -- each a count taken from one census walk, zero-inclusive,
and all five drawn as a dash when this face was not handed its own record, because "nothing
happened" and "nobody told me" are two facts. The header line above it carries no figure at
all now: it named the same denominator the band states, at a fifth of the size.

**The runs are the boxes.** `groupRuns()` already collapsed consecutive identical entries;
each run is now a bordered box whose head states the shared address, how many calls it
collapses, and its standing as a chip. Ungrouped entries collect into one box (`calls  8
rows`) so the list is an object rather than loose rows. An empty window keeps its border and
says `0`; a window never handed its record keeps its border and says `--`.

**Two `req/97` section-4 defects, cured.** (1) `wire code: IDEMPOTENCY_CONFLICT` and
`{"name":"get_everything_i_wish_for"}` were drawn literally -- `req/96` axis B's hard rule
scores that 0. Neither is deleted: the surface now says `no route by that name is one this
window can reach`, and the server's own spelling of both is under a new `reference` control
("the server's own words, unedited"), the cure `faces/atlas` built for its own leak. Held by
a drawn-surface scan with a negative control that fires the same two patterns on what was
kept. (2) The status column wrapped mid-word. The cause was never the viewport: the track is
`minmax(0, N)`, so it was the same width at 1426px and at 720px. `tools/shoot.mjs` gained a
reading that measures the widest word against the width the column gave it -- **117px wanted,
112px given** -- and the ceiling is now 9rem with the status on its own line. The first
version of that reading returned 0 on a page whose picture plainly showed the break (a
Range's *bounding* rect over an already-broken word is the union of its fragments, so it
reports the column's own width); it sums the fragment rects instead.

**`req/103` finding 2, on this face.** An open disclosure was shut by every repaint. The
audit reached it on `ledger` by clicking a row; here the way in is the poll -- a reader opens
the legend and the next call the window records closes it. `mount()` reads the open state off
the live document before clearing it and hands it back through `read(notices, open)`.

**Two unexplained totals, refused.** The `tally` control counted the same words the band now
states. It is gone as a control: three of its words are figures in the band, and the rest
(zero-inclusive, plus unrecognised words and the not-a-record bucket) moved inside `legend`,
where a counted table already was. The test holds the property rather than the arrangement --
every word of the closed set is counted in exactly one of the two.

**A gate that had never been run.** `node tools/gate.mjs` printed nothing and exited 0: the
file had no command-line block and every check was only ever reached from `test/gate.test.mjs`,
which draws one state. Given one, it went red immediately on `one-meaning-one-mark` over the
overflow page -- the only state that draws a standing chip, whose `data-means` sat on the
wrapper and whose `data-mark` sat on the glyph inside it, so the wrapper read as a second
mark. The check now ignores a node that carries a meaning and no mark, and a new test fires
the tree checks at every shipped state. The shared part was fixed independently the same day
(`parts/` commit `4b09865`, another lane, same defect found the same way), so this face
reaches that line today only if a future part wraps a mark in a labelled span again.

**Verified** (this lane's own numbers, no independent re-run): `node --test` **73 pass / 0
fail**; `node tools/gate.mjs` **14 checks, 0 failing, over 3 drawn states, exit 0**;
`node tools/bench.mjs` median **8.621ms** against a 300ms budget (1,000 entries);
`node tools/shoot.mjs` 7 captures, `overlaps=0 repeated=0 oversize=0 filled=0
clippedWithoutFull=0 wrappedInsideAWord=0 horizontalOverflow=0 underTapBudget=0` on every
one; `node tools/browser-mount-smoke.mjs` **ready, 285 elements, 8 entries, 8 reach controls
all disabled and all titled, exit 0**. Scroll-container grep over the shipped source
(`overflow`, `max-height`, `height:`): **10 hits, 0 scrolling containers** -- 8 are
`overflow-wrap:anywhere` and 2 are `overflow:hidden` on the fixed 72px time cells, whose full
value is in `title`/`data-full`. This face adds no scrolling container and has none.

**Left behind, named.** (Superseded 2026-08-25 -- see "Round 5" below: the eight are gone.)
The eight `reach` controls are still eight disabled buttons in a
column; they are honest (C-7 gives this face no way to address another face) but a reader
meets eight invitations that cannot be taken, which is worse at eight than it was at one.
Nothing on this face carries a standing hue: the only ink this application spends on a
standing belongs to the verdicts, and a call the server refused is not a candidate the engine
denied -- so at arm's length this screen is told apart by figure size, box edges and marks,
never by colour. That is a decision, not an oversight, and it is the reason this face will
always read cooler than `held`.

## Seen and not seen (denominator)

- The retired tree's notice face: **0 lines**, by construction. Its `FACE.json` was not opened either -- the `consumes: []` requirement in `req/03 §3-1` was read from the requirement document.
- `faces/ledger/*` in this repository: **0 lines opened.** The shared shape (declaration/binding/face-module/index, `tools/gate.mjs`'s check roster, `tools/fixture.mjs`/`tools/shoot.mjs`'s structure) was followed from the dispatching lane's description of the pattern and from this lane's own reading of `membrane/src/membrane.mjs` and `shell/kernel/shell.mjs` (the actual producers of a `notices` entry), not from `faces/ledger`'s source.
- `gx-api` handlers: **0 lines.** This face never names a member of a response body; every field it reads off an entry is membrane/shell envelope metadata (`outcome`, `status`, `gx_code`, `reason`, `detail`, `requested`, `said`), not domain data.
- `glovrex_web/req/phase/app_notice.req.md`: **0 lines** (not located; may not exist under that name).
- Real `gx serve`: **0 calls** from this face, structurally -- see C-7.

## Round 5 (Owner #348 / #349, 2026-08-25)

**The icon floor.** Eight call sites drew a mark below the readable floor: seven at 15 and
one at 14. The 15s were this face's alone on the tree, and they came from the row-line type
scale (`.gx-row-line` is 15px) -- a mark had been sized to match the text beside it, which
is the wrong question, because a letterform at 15px is a shape a reader already knows and a
24-unit mark with a 2-unit stroke at 15px is a shape nobody can resolve. Every size now
comes from `MIN_READABLE`/`MIN_ACT` through `binding.mjs` (`P.floors`); a disclosure's fold
mark takes the act floor, being the one thing here a hand aims at. `tools/gate.mjs` gained
`no-hand-picked-mark-size`, which is stricter than the shared floor gate: a literal below 16
is not the failure, a literal at all is.

**Right-click, and the eight dead buttons.** This face declares no acts and never will
(C-7), so the menu's case is "copy value" -- the call, the wire code and the time each get
one, plus the whole row -- and it says in a sentence that nothing on this screen can be
sent, rather than holding four copies and no verbs and letting a reader infer it. The eight
`reach` buttons are retired: the column a row has for a thing a hand can do was spent on a
thing no hand can do, and a right-click menu on such a row would have been a second way to
arrive at the same refusal. Their reason is kept, once, as a declared omission. What stands
in that column is `copy row`, which works. Three properties are structural rather than
guarded: one state slot (a second right-click overwrites, so two menus cannot exist), the
menu is part of the tree a repaint rebuilds (so a repaint cannot orphan one), and Escape and
a press away both clear that slot. All three are fired in a real browser through the
renderer's own input pipeline in `tools/browser-mount-smoke.mjs`.

**Text.** Four weights by role (`data-type`, held mechanically by a test that walks the tree
and refuses a weight set without a role). The declared omissions were being drawn twice, in
two different controls, on one screen; the `omitted` heading restated the control it sat
inside; the budget count was stated three times. Drawn characters on the representative
fixture: **11,621 -> 9,565** (empty 10,637 -> 8,535; overflow 19,590 -> 17,493). Two of the
legend's mark rows explained themselves with a directive number, two function names and
another face's directory -- vocabulary only this codebase can look up, on a product surface.

**Instruments.** `clippedWithoutFull` examined only nodes carrying a `data-role`; it is over
every element holding text of its own now, and the same widening was applied to the
mid-word-wrap reading, which then fired five false positives per page on single-line cells
(a word measuring 50px in a 49px box) until it was made to require a cell that actually
wrapped. A new orphan-last-line reading caught a stranded "in" the moment the control row
was made to share its width. Captures are cut to the page (`contentHeight`), not to a
1400px viewport holding 700px of content.

**Verified** (this lane's own numbers): `node --test faces/notice/test/*.test.mjs` **95 pass
/ 0 fail**; `tools/gate.mjs` **18 checks, 0 failing, over 4 drawn states**; `tools/bench.mjs`
median **11.248ms** against 300ms (1,000 entries; 8.6ms before, and the difference is this
round's per-row control); `tools/shoot.mjs` 7 captures with `overlaps=0 repeated=0
oversize=0 filled=0 clippedWithoutFull=0 wrappedInsideAWord=0 orphanLastLine=0
horizontalOverflow=0 underTapBudget=0` on every one; `tools/browser-mount-smoke.mjs` **exit
0**, 8 offer controls all live and all titled, 0 reach controls, and an eight-step
interaction pass whose menu counts read 0,1,1,1,1,0,1,0.

**Left behind, named.** At 1280px the address column takes every spare pixel while the
outcome column, capped at 9rem, still wraps `asked, not yet answered` -- about 700px of
empty ground beside a column too narrow for its own words. The cap is what the mid-word
measurement asked for at 720px and nothing yet makes it grow when there is room.
