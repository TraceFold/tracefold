# faces/ledger — **what happened, in order** (F-1)

> **status**: implementation landed 2026-08-24. `node --test "test/*.test.mjs"` = **61 pass / 0 fail / exit 0**. Real-renderer captures = **7** (3 pages x light-narrow / light-wide / dark), every reading clean.
> **origin**: written from `req/03_FACES_REBUILD.md` and `req/04_PARTS_REBUILD.md`. **COPY HARD BAN kept structurally**: the retired tree's `faces/ledger/*` was never opened — not the module, not its declaration file, not its tests. The inputs were the contract table in `req/03 §3-1`, the negative truth ledger in `req/03 §4`, `req/05` RT-05..RT-08, the membrane's public surface, and the parts' own source. **External OSS consulted: none** (`§24d` ledger: 0 rows).

## Retrofit r5, 2026-08-25 (Owner #348 (2)(3)(4)(5), #349 (3))

Five things, and the number each one is.

**The other button.** Every row and every act now answers a right-click with a menu, and
the menu is not a second opinion about what the row offers: `offeredActs()` is the only
function on this face that turns the declaration into a list of offers, and the gutter
and the menu are both handed what it returns. A menu act button carries the same
`data-act` / `data-target` pair a gutter button carries, so the press goes down the same
branch of the same handler into the same queue — measured: a menu commit and a gutter
cancel pressed in the same tick are 2 calls and 2 log entries, which is the reading r4
used to prove the lost update (2 sent / 1 written) and then cure it. A withheld act is
in the menu, disabled, carrying the declaration's own sentence; an act in flight is
dimmed in both surfaces at once. The menu is state, not an overlay a handler put on the
page, which is what makes the three properties #348 asks for hold by construction: one
slot, so a second right-click cannot stack a second menu; drawn from state, so a repaint
cannot leave one behind; dismissed by Escape (on the document), by a press anywhere else
in the face, and by a press outside the face entirely.

**It is in flow, under its row, and not pinned to the pointer.** This face's own
`nothing-out-of-flow` gate exists because the one defect this package has shipped (N-1)
was an absolutely positioned element drawn over a row. A pointer-pinned menu would have
to be positioned, would overlap whatever it covered, and would be invisible to the
overlap reading every capture here is checked with. In flow it is measurable and
keyboard-reachable in reading order. Cost: the rows below it move down while it is open.

**Copy value takes the member, never the drawn text.** The `at` cell draws a declared cut
of an ISO-8601 timestamp, so a copy of what the cell says would hand back something that
is not the value, quietly. The clipboard gets `record.at` in full, and the copy goes
through the *same queue* an act does — a clipboard promise writing `state` on its own is
exactly the shape that lost an act in r4. Whether it worked is on the screen in the
shell's own vocabulary (`data-copied` / `data-copy-failed`); with no clipboard in the
window it says so rather than looking identical to a copy that worked.

**Icon floor (#348 (3)).** Three call sites were under it: `12` (provenance fold), `14`
(fold marks, held chip). All marks are now asked for by name — `P.minReadable` (16) and
`P.minAct` (20) off the sheet — including the band's, which was `18`, a number picked by
eye beside a 22px figure. **The measured cost, which is the point:** the 20px act mark
made `commit` need 78px and `escalate` 80px inside a 75px gutter button — 2 of 4 acts
drawing their word out past their own border at *both* viewports, on captures that read
zero on every existing probe. `parts/tools`' `GUTTER_WIDTH` was fixed to 92px at
`821fc95` by the `faces/held` lane; `tools/shoot.mjs` now carries the reading that would
have caught it (`overflowingControls`), and it is 0 on all 10 captures.

**Row height, measured and stated.** A settled row is **49px**; a held row is **122px**
for the same 49px line, because the act gutter stacks three 38px buttons with 4px between
them. The 20px mark did *not* make it worse — an act button is `min-height: 36px` with
8px of vertical padding, so a 20px mark fits with nothing to spare, and one more pixel of
mark would grow every act button on every face. **73 of those 122 pixels are empty**, and
two candidates cost 244px where two lines would cost 98px. That is not defensible for a
list whose whole job is to be scanned, and it is `parts/src/receipt-row.mjs`'s to change,
not this face's. The shape proposed in the report: lay the gutter horizontally, marks
only (they are at `MIN_ACT` precisely so they read alone), with the label in `title` —
which needs a measurement of `GUTTER_WIDTH` against the fingerprint clipping the original
84px was chosen for.

**Text (#348 (4)).** Three weights, named and applied mechanically rather than at
whichever call site thought of it: `WEIGHT.figure` (700) on every number, `WEIGHT.label`
(500) on every word naming one, `WEIGHT.body` (400) on prose. The header line's
denominators are mono figures with their nouns beside them, which is the band's own rule
applied to the one line above the band. Line breaking: every `overflow-wrap: anywhere` in
this face is now `break-word`, and a gate holds it at zero — `anywhere` lets a word that
would have fitted be broken anyway, which is what put `repor / t.md` on two lines.
Redundant words removed, **1256 -> 1225 visible characters** on the full page (and 792 ->
762, 897 -> 868 on the other two): three section headings that repeated the label of the
control the reader had just pressed (`claims`, `consistency`, `omitted`); six `--`
separators between a control's label and its hint; a requirement number (`req/768 F-I`)
printed in the legend of a product surface; a fold summary that said what its own control
and hint already said twice; and the band's `admitted / denied / escalated`, which taught
a second vocabulary for the three words every row badge already says — now `admit / deny
/ escalate`, which is also the only way the label fits (`escalated` overran its column by
8px at 720px and drew as `ESCALA...`).

**One shape per job (#349 (3)).** `aside` and `plain` were two functions differing in a
colour and two pixels; they are one `line()` with two roles. The legend's mark tally, its
prose, its not-drawn set and the omitted list were four copies of one grid; they are one
`gridLine()` with a column track. `heading()` was deleted with its last call site.

### What this round measured and did not fix

- **5 texts are cut inside a cell at 720px with the full word nowhere else on the page**:
  `Admit`, `Deny`, `Escalate`, `delete`, and the undefined-verdict sentence. Every
  capture of this face has reported `clipped=0` while `A...` was on screen, because the
  cell's own `scrollWidth` equals its `clientWidth` — the ellipsis is on a child span.
  `tools/shoot.mjs` now reads that too (`ellipsized`). The widths are
  `parts/src/receipt-row.mjs`'s `COLUMNS` / `SCAN_WIDTHS`, not this face's.
- **The path cell breaks mid-token** (`/work/repor` + `t.md`). A path offers a browser no
  break opportunity it recognises, so `overflow-wrap` breaks it wherever the line ends.
  The fix is a break opportunity at the separator (a `<wbr>` after each `/` in
  `valueCell`), and that is a parts file.
- **The menu's own controls inherit no operability rules.** `parts/src/surface.mjs` keys
  hover, press and cursor to `[data-part="act-gutter"] button`, so a second act surface
  either copies the colours inline (what this face does, with nothing to outrank) or lies
  about its part name. A role-based selector would fix it for every face.
- **There is no `act/copy` in the glyph sheet**, so the copy item is the one control in
  the menu with no mark. A face may not invent vocabulary and may not reuse another
  mark's meaning, so it is a word until the sheet has one.

### Gates fired red this round

`no-raw-corner` (found `'border-radius': '4px'` in this face) and `no-mid-word-breaking`
(found 4 `overflow-wrap: anywhere`) went red against the shipped source before they went
green; `no-raw-motion` was fired on two planted strings. Six of the new assertions were
fired by breaking the source they are about, one at a time, with byte-identical restore:
the menu drawing its own act list, the copy writing state outside the queue, the menu
slot becoming a list, Escape not being listened for, click-away not dismissing, and the
copy taking the drawn text of the cell instead of the member.

## Retrofit r4, 2026-08-25 (Owner #340; the interaction findings in `req/103`)

Owner #340 read this screen as monotone, hard to take in at a glance, and hard to
operate. Eight things changed, and each one is a count or a press rather than an
adjective.

**A band of five figures at the head.** `settled 5 / admitted 2 / denied 1 / escalated 1
/ held 2`, every figure counted from the rows this render is about to draw, each standing
in its own ink and behind its own mark. Zero is drawn (the `ledger-held` capture reads
`0 admitted / 0 denied / 0 escalated` against `1 settled`, because that one row carries a
word the engine never promised, and it is counted under none of the three). **A half that
was not read draws a dash and never a nought** -- the `ledger-unread` capture is five
dashes, which is the same distinction this whole face is built around, stated for the
first time in a figure rather than in a sentence.

**Three boxes where there were three headings.** `settled` (n records), `held` (n
candidates, wearing the filled `held` standing on its head) and `acts` (n entries). The
two halves are counted in two different words on purpose: a candidate is not a record. An
empty half keeps its border and says `0`; an unread one says `--`. One shape,
`boxSection()`, at all three call sites.

**A measured strip at the foot.** `render N ms` is `performance.now()` around the tree
build in `view()`, taken before the strip that reports it is built; `read` is the word
whoever knows gave -- the fixtures state `a stand-in, not an engine`, so no capture of
this face claims an engine was on the other end of it, and a screen that read nothing
prints a dash there.

**Three interaction defects, from `req/103`'s 101-operation audit.** (1) Two presses of an
act button in the same tick used to send two acts and record one: both read the same
`state`, both appended to the same log, the second write overwrote the first. Acts are now
queued, and the read of `state` happens inside the queued step. Reproduced before the fix
(2 commits reached the port, `data-count` said 1) and after (2 and 2). (2) An act in
flight now dims its own button and says why, instead of looking idle. (3) Which panels a
reader has open, and which row the pane is describing, are carried in this window's own
state and survive a repaint -- `read()` no longer throws away what the window decided when
it replaces what the server said.

**Numbers, this lane's own**: `node --test "test/*.test.mjs"` **96 pass / 0 fail**;
`node tools/gate.mjs` 14 checks, 0 fell, over 3 states (it prints them now -- run bare it
used to print nothing and exit 0, which is what a gate that had done nothing also does);
`node tools/bench.mjs` median **70.1ms** for 1,000 rows against a 300ms budget. Read that
figure with its spread and not as an improvement: the same bench read 74.9ms before this
lane and 85.7ms with this lane's work on the previous hour's `parts/`, and single runs of
five samples on this machine move by more than the difference between all three. What the
three readings do settle is that the band, the boxes, the strip and the per-row act copy
did not move this face into a different order of magnitude;
`node tools/shoot.mjs` 7 captures, `overlaps=0 repeated=0 oversize=0
filled=0 clippedWithoutFull=0 underTapBudget=0 overflow=0` on every one;
`node tools/browser-mount-smoke.mjs` ready, 448 elements.

### `[ ]` left behind by this lane

- `[ ]` **The accent pass does not reach a row or an act button.** `parts/src/surface.mjs`
  asks for `cursor:pointer` on rows and live act buttons, the accent as their ink and
  border, a hover step, and the accent bed on the selected row. `parts/src/receipt-row.mjs`
  writes `cursor`, `color`, `border` and `background` **inline** on those same elements,
  and an inline declaration beats a rule. Measured in a real window on this face's own
  fixture: row `cursor: default`, row background `rgb(246,245,242)` (the page), live act
  button `color: rgb(29,29,29)` and border `rgb(232,232,232)` -- while `--act` is declared
  `#1d5f8a`. Six of the seven operability declarations are inert. Not this face's file to
  fix; the fix is in the parts lane and is either dropping the inline pair or `!important`.
- `[ ]` **A held row costs 122px against a settled row's 49px.** The act gutter stacks
  three buttons vertically at a 36px floor each, so every held row carries ~73px of empty
  space beside it. This is the worst remaining defect on this face and it lives in
  `parts/src/receipt-row.mjs`'s `actGutter` (`flex-direction: column`), not here.
- `[ ]` **Two band labels ellipsise at 720px.** `admitted` wants 71px in a 69px column and
  `escalated` wants 77px in 69px; five equal columns at 720px with the band's own 20px
  padding do not fit the longer words. Nothing is cut at 1280px. The figure and the mark
  are never cut, and the whole word is in the segment's title. Fixable only by a shorter
  noun (a worse word) or by the band's own padding (not this face's file).
- `[ ]` **The build hash in the strip is one commit behind.** It reads `b47b7ac +changes`
  while HEAD is `4fca679`; `parts/generated/build.generated.mjs` is regenerated by the
  parts lane, not by this one.
- `[ ]` **Panels inside a panel are still native-only.** The provenance fold's own
  `<details>` (inside `where from`) is not carried in state, so it alone still shuts on a
  repaint. The six controls and both `order:` folds are carried.

## Finishing lane, 2026-08-24 (req/38 SS551/SS558/SS576, app req/97 Pass 2)

**SS558 body-text floor (this lane's first pass)**: every body-text call site that read
`T.meta`/`CONSUMED.meta` (12px) now reads `T.record`/`CONSUMED.record` (14px) -- a token
swap, not a new literal (`Z-2`'s consumed-roster discipline is unaffected, both names were
already in `parts/src/tokens.mjs`'s `CONSUMED`). One fixed-pitch column was deliberately
left at the smaller `CONSUMED.time` (13px) in the sibling `faces/notice` -- documented
there, not here -- because that column's width was measured against 13px metrics
(`req/100_PLACEMENT_SPEC.md`) and bumping it without re-deriving the column budget would
reproduce the exact N-4 clip defect req/03 found. `faces/ledger`'s own fixed-pitch columns
(`at`, `fingerprint`) were left untouched for the same reason.

**What changed next** (this lane's own scope; the shell-chrome half of the same brief is
`shell/README.md`'s Docker-IA lane section): two coordinator-relayed repair rows, both
additive re-skins, no new mechanism. (1) **Data before prose**: `halfOf()` used to draw
`order:`/`order-reason`/`verifier` sentences between a half's heading and its rows --
open prose ahead of the first table on every load. Reordered so the rows draw immediately
after the heading (and after the short `empty`/`all-dropped` state facts, which are load-
bearing rather than explanatory), and the order/reason text now lives in a `peripheral()`
fold (`order: <applied word>`, closed by default) *after* the rows -- the same disclosure
pattern `legendSection()` already used, applied to the one place it hadn't been yet.
C-6's own contract (`order` + reason "printed" on the section) is unchanged in substance,
only in position; `test/ledger.test.mjs`'s C-6 check reads `textOf(section)`, not position,
and still passes. (2) **Explicit size hierarchy, not one flat size**: this lane's earlier
SS558 pass (below) bumped every body line from `T.meta` (12px) to `T.record` (14px)
uniformly, which met the floor but left the whole face reading at one weight under the
17px headings. `parts/src/receipt-row.mjs`'s `note-summary` line (the fact a reader opened
a row *for* -- "verdict in full: X" / a held row's own reason) now draws at
`CONSUMED.head`/`CONSUMED.ink`/weight 600, a free-flowing line with no fixed-pitch column
to clip, so the larger size costs no width budget the way a row cell's own font would.

**Verified**: `node --test` 61/61 unchanged (no test asserts row *position*, only presence);
`node tools/fixture.mjs && node tools/shoot.mjs` -- all 3 fixture states, both themes:
`overlaps=0, clippedWithoutFull=0` unchanged from before this lane's edit (the hierarchy
change sits outside every fixed-pitch column, by design, specifically to avoid the clip
regression the notice face's own SS558 pass hit and fixed in the same session -- see that
face's README). `req/97` Pass 2 re-scored this surface (`faces/ledger/fixtures/shots/`):
**verdict 0 → 3** (the 0 was an earlier lane's `undefined mark` defect, already fixed
before this lane started; this lane's own contribution is the reorder + hierarchy, scored
on top of that fix). Worst defect named there: the collapsed `order:` fold now restates
words a reader can already see from the row order, rather than adding a new fact.

## Re-skin lane, 2026-08-24 (req/97 RC-1/RC-2/RC-3 repair, composition A)

**What changed**: row grammar moved to composition A (`lifecycle` glyph, `effect` glyph+word,
verdict badge with canon check/cross/person marks, a `fingerprint` cell, `seal`, `path`);
the ~120-word repeated per-row prose (how a serial is cut, why it is not a proof, the default
seal claim) collapsed into one `legend` `<details>`, said once; layer vocabulary
(`membrane`) removed from the one visible line that carried it (`UNDRAWN.next_cursor`);
`clippedWithoutFull` boilerplate moved out of the row into the same legend; the H1 banner and
the always-open "why" paragraph were removed/folded per the Docker-CLI-grade ruling (Owner
#284 SS549); act buttons now lead with a coined `act/*` glyph and hit a 36x36px tap-target
floor (Owner #286 SS553); the real-window mark regression (§"regression root cause" below) is
fixed. See `req/100_PLACEMENT_SPEC.md` for column-width budgets, the spacing scale, and
per-component placement rationale.

**Regression root cause**: `mount()` never called `installSheet()`. The static fixture writer
(`tools/fixture.mjs`) built its own page around `toHtml(parts.sheet())`, which is why fixtures
drew marks while a real mount — headless smoke and the real Chrome window alike — drew none.
Fixed by calling `P.installSheet(doc, P.element.render)` at the top of `mount()`, guarded on
`doc.getElementById`/`doc.body` so the structural test stand-in (which has neither, by design)
is unaffected. Verified: real-window PNGs before the fix show zero canon marks; after, the
same three-row fixture shows a pencil, a trash can, a checkmark, a cross, and a person mark, in
both themes (`record/real_window_{light,dark}.png`, regenerated).

**5-principles self-assessment** (`INHERITED_PRINCIPLES.md §3c`):
1. **template-form** — PASS. Composition A is the Owner-adopted design (`req/09 §3`); this
   lane implements it as an evolution of the existing 7-column grid (now 8, `fingerprint`
   added) rather than inventing a second row shape.
2. **lightweight+bench** — PASS with a figure, run and confirmed this lane:
   `node tools/bench.mjs` → **median 76.50ms** for `view(state)` over 1,000 settled rows
   (samples 62.9/75.0/76.5/79.3/119.2ms, budget 300ms). The harness (`tools/bench.mjs`) was
   written by a concurrent lane on the same 5-principles mandate, not authored here, but the
   code path it times is this lane's own — every row it renders goes through the composition-A
   `receipt-row.mjs`/`glyph-sheet.mjs` changes landed today.
3. **english+comments** — PASS. All new/changed code is English-commented; error/success
   text audited for the removed "undefined mark" fallback (verdict-badge.mjs now names the
   word that actually arrived instead).
4. **always-CRUD** — PARTIAL. Reads (R) are the whole of what this face does by design (C-7,
   `03_FACES_REBUILD.md`); the three offered acts (commit/cancel/undo) are the U/D-adjacent
   surface this face declares (`declaration.mjs ACTS`) and now carry a glyph+tap-target-sized
   control. Create is out of scope by design (`NG-5`, req/09 §7) — this face is a read/decide
   window, not an authoring one.
5. **DB-principle** — PASS. store=source is unchanged: every value drawn is read fresh from
   the port on each `read()`, nothing is cached or reconstructed client-side, and the fixture
   HTML files under `fixtures/` are declared-rebuildable (`node tools/fixture.mjs`), never
   hand-edited.

**What was explicitly not done, and why**: the Docker-IA shell chrome relayed mid-lane (Owner
#285 SS551 — left rail, persistent bottom status bar, breadcrumb+tabs, copyable command
blocks) is `shell/kernel` scope, not `faces/ledger` scope — building it here would mean a face
reaching into the shell's own layout, which is exactly what `W1` (the shell does not know a
face's contents, and a face does not build the shell's chrome) exists to prevent. A from-
scratch destructive rewrite was authorized (Owner #287 SS555) but declined: the incremental
path landed 371/371 green (354 unrelated to the whole-repo copy-gate + 1 known unrelated
failure, see below) with a verified regression fix and verified real-window captures; a
rewrite at this point would have traded a verified, tested state for an unverified one with no
budget left to re-verify it. The SS553 ~50%-text-count target is reported as a measurement
(`visibleTextChars`, `tools/shoot.mjs`), not claimed as met — no pre-edit baseline was captured
before this lane started editing, so only the current, post-edit number is a real measurement;
see the session report for the qualitative accounting of what moved into the collapsed legend.

**Also relayed and not built this lane** (Owner #288 SS558 — 14px+ body text/larger primary
data, abstraction of near-duplicate rows into one drill-down row, at-a-glance region
self-test): landed after this lane's verification pass was already closing out (371 tests
run, screenshots regenerated, real-window regression proven fixed and re-proven after a real
bug found in interaction testing). Implementing it properly needs another full edit-verify
cycle across both faces' typography and the row-collapsing logic in `receipt-row.mjs`/
`notice.mjs`, which would not fit inside this lane's remaining budget without either rushing
it unverified or re-opening every invariant already re-proven above. Filed as an honest gap
for the next lane rather than attempted partially.

**Unrelated test-suite note**: `membrane/test/discipline.test.mjs`'s D5 copy-gate fails against
freshly-written `tools/bench.mjs` files in five directories (membrane/parts/shell/ledger/
notice) that this lane did not write — they carry a concurrent lane's citation comment
(`faces/ledger/tools/bench.mjs:2`, referencing this README) and are mid-flight on the same
5-principles mandate. Not touched here (no-delete, shared-worktree discipline); excluding that
one file, the full suite is 354/354.

```
node --test "test/*.test.mjs"     # 61
node tools/fixture.mjs            # writes fixtures/*.html
node tools/shoot.mjs              # real renderer: captures + readings
```

**`record/real_window_*.png` (2026-08-24 repair lane declaration, app req/98 V-13):** regenerate with `powershell -File ../../shell/tools/real_window.ps1` against this face's mounted window. These are **not reproducible pixel-for-pixel** -- they are live `CopyFromScreen` captures of a real OS window (font hinting, DPI, whatever else is on screen at capture time), unlike `fixtures/shots/*.png` which come from a controlled headless render. Declared here rather than left implicit next to the regenerate command above.

Zero dependencies, node builtins only. No `package.json`.

## The shape

| file | what it is |
|---|---|
| `declaration.mjs` | the only place a server method is spelled out; consumes / sends / withheld / marks / order / undrawn / tests |
| `binding.mjs` | the seam to the parts. The face is *built with* parts and can be built with others |
| `ledger.mjs` | `createFace({parts})` -> `{ mount, read, act, view, toRecord, callerFor }`. `mount(host, port, notices) -> unmount` |
| `gate.mjs` | 14 machine checks: 10 over shipped source, 3 over what was drawn, 1 over the disk |
| `index.mjs` | the one door a shell mounts |
| `tools/fixture.mjs` | three states written out as pages a browser can draw |
| `tools/shoot.mjs` | the same pages in front of a real renderer, photographed and measured |

## The three properties that are load-bearing

**A list that could not be read is never an empty ledger.** Every half is `data-state="unread"` or `"empty"` and the two carry different sentences. Three tests fire it — one per non-answer (`failed`, `refused`, `absent`) — and each asserts the empty-ledger words are *absent* from the page.

**A row that has been written is never edited.** Undo does not rewrite its target: the act is sent, the list is read again, and the child row appears because the server appended it. The test slices the target row's markup out of both renders and asserts byte equality, then asserts `data-child-of` appeared. Records are frozen and no shipped line assigns into one (`rows-are-not-edited`).

**A count is never given alone.** Each half states rows drawn of rows received, the number of requests the walk took, whether it stopped at its budget or on a repeated cursor, and every dropped item with the reason the order gave for it.

## The contracts (`req/03 §3-2`)

| # | how it is held | state |
|---|---|---|
| C-1 | the caller refuses a name the declaration does not hold, **before the port is reached** — so this is structural, not a grep. Declared names are checked against the membrane's own route table | [◐] |
| C-2 | `sends` / `withheld` split the declaration; the withheld act is drawn dimmed with its reason on screen and is never sent | [◐] |
| C-3 | `rows.draws=true`, `reports_denominator=true`, and an eight-line `undrawn` list, all printed in a **what is not on this screen** section | [◐] |
| C-4 | every `data-mark` drawn is in the declared set; the check **refuses to pass on an empty population** | [◐] |
| C-5 | meanings are declared as ids and carried as `data-means`; one meaning with two marks goes red. The two halves take their marks from the parts' `HALVES` rather than choosing again | [◐] |
| C-6 | `order.position` + reason, and `rows.order` + reason, both printed above the rows. A substituted order states the substitution | [◐] |
| C-7 | **not this face** — it declares seven methods. `silent_face` names which face is the control | [ ] elsewhere |
| C-8 | the declaration names its three tests and the gate checks each is on disk | [◐] |

## The negative truth (`req/03 §4`, `req/05` RT-07/RT-08)

| # | how it is held | measured | state |
|---|---|---|---|
| AC-F1 | nothing in the face is positioned; the note is an ordinary block that follows its row and carries an opaque background of its own | `overlaps=0` on all 7 captures, **and looked at** | [◐] |
| AC-F2 | one builder, one sprite, one row per record | `repeatedRows=0`, `sprites=1` on all 7 | [◐] |
| AC-F3 | size is a required argument of `glyph`; omitting it throws rather than falling to a default | `oversizeGlyphs=0`, `filledGlyphs=0` on all 7; 20 glyphs on the full page | [◐] |
| AC-F4 | the captures were **opened and read by eye**, not only measured | **5 of 7** viewed in full: `ledger_narrow`, `ledger_wide`, `ledger_dark`, `ledger-held_narrow`, `ledger-unread_narrow` | [◐] |
| AC-F5 | one new entry added below | — | [◐] |

### What the renderer found that the tree could not (this face's own instance of `§24c`)

1. **The claims read as five overlapping text pairs.** Claim and detail were two inline spans in one grid cell; a wrapped inline box reports a rectangle covering every line it touches, so two of them read as overlapping whether or not anything is painted over anything. Fixed by making them blocks — **a reading that cannot tell a real overlap from a wrapped one is useless on the defect it exists for**, so the ambiguity was removed rather than excepted.
2. **Four values were shown cut off with the whole of them nowhere on the page.** The first clip-risk rule used one length for the whole row and missed the narrowest column (`at`, 72px, holding a 20-character timestamp). Now the budget is derived per column from the width that column declares, and a new reading — `clippedWithoutFull` — holds at **0**: every value that gets cut off on the line appears in full in the note underneath.
3. **A held row's note said two different things under the name "seal"** (the declared hole, and the seal claim). The claim line is now omitted where a hole is declared, and a test holds every note's line names distinct.

None of the three were visible to any assertion over the tree. All three are now.

### Negative truth ledger, this face's addition

| # | must not recur | found | AC |
|---|---|---|---|
| N-4 | a value clipped on the row line with its full text nowhere on the page | this face, first capture, 4 cells | `clippedWithoutFull=0` in `tools/shoot.mjs` |

## Every gate has gone red at least once

Ten source gates (`no-network`, `no-foreign-import`, `no-verification`, `no-actor-named`, `no-colour-literal`, `no-borrowed-symbol`, `nothing-out-of-flow`, `no-method-literals-outside-the-declaration`, `no-dynamic-code`, `rows-are-not-edited`) each have a planted string in `test/gate.test.mjs` that makes them red. So do the three drawn-tree gates and the on-disk one. The mark and meaning checks additionally fail on an empty population, because a rule that passes when nothing was drawn is not a rule.

## `[ ]` — not done, and not called done

- `[ ]` **No real-window mount.** `mount` is exercised against a stand-in document (`test/dom-stand-in.mjs`), which proves only that the face asks a document for the right things. The parts import node builtins (`tokens.mjs` reads the roster off disk), so the drawing code cannot be loaded in a browser without a build step this package does not have. **`req/02` W15 is therefore open for this face**, and every visual claim above comes from the fixture in front of a real renderer instead. Closing it needs either a node-free token module or a bundling step — a decision for the shell lane, not this one.
- `[ ]` **Independent re-run `[●]` = 0.** Every number here is this lane's own. `req/03` AC-F0 (match the reference tree's PASS count) is **unmeasurable**: `req/05` carries no per-face PASS count for the ledger — RT-05 is 10/10 across five faces and two themes for rail transitions, RT-06 is 6 of 8 across all faces. There is no denominator to match, so AC-F0 stays `[ ]` rather than being declared met against a number that is not there.
- `[ ]` **Not registered with the harness.** `tools/faces.json` (the declared mount set the T2 tier runs on) still lists only fixtures. This lane's write scope was `faces/`, so the row was not added. Until it is, `verify-all` does not see this face.
- `[◐]` **Row-render bench added (2026-08-24 repair lane, app req/98 V-1).** `node tools/bench.mjs` times `face.view(state)` (the real exported paint function) over 1,000 settled rows, median of 5, persisted to `.bench/report.json`, hard-red budget (300ms). **Mount ms is still unmeasured** -- host-attach + first-read latency needs a real document or renderer and remains open. So the second principle is now `[◐]` (row-render measured, mount not), not the prior flat unmet.
- `[ ]` **The act path is not wired end to end.** `act()` is tested against a stub port and against a throwing one; it has never reached a real `gx serve`. Bodies are sent empty because `gx-api`'s request members were never read out of the crate — which is also why `escalate` is declared, offered, and withheld.
- `[ ]` **`at` is too narrow for what goes in it.** Every row on the full page opens its note because the 72px time column cannot hold an ISO timestamp. That is honest, and it also means the fixed-pitch scanning line the row was designed for does not survive contact with real data. The fix is a column width or a stated shorter time format, and both live in the parts lane, not here.
- `[ ]` The `<details>` disclosure triangle in the provenance fold is a UA marker, not a bespoke mark — carried over from the parts lane's own open list.
- `[ ]` Two of the seven captures (`ledger-held_dark`, `ledger-unread_dark`) were measured but not opened.

## Negative-control lane (2026-08-24) — AC-F1..AC-F4 falsifiers performed

One AC at a time. Each file was backed up byte-for-byte before its break and restored from that backup (no hand-reverting); `cmp` confirmed byte-identical restore in all four cases.

| AC | break performed | red evidence | restore |
|---|---|---|---|
| AC-F1 | `parts/src/receipt-row.mjs` `note()`: `background: CONSUMED.page` → `'transparent'` | `node --test parts/test/receipt-row.test.mjs` → 1 fail: `AssertionError` on `block.attrs.style.includes('background:...')` (test "the note carries its own opaque background...") | `cmp` byte-identical |
| AC-F2 | `faces/ledger/ledger.mjs`: row map `.map(record => rowBlock(...))` → `.flatMap(record => [rowBlock(...), rowBlock(...)])` (each row drawn twice) | `node tools/shoot.mjs` → `repeated=7` (ledger) and `repeated=3` (ledger-held) on every capture, was `repeated=0` at baseline | `cmp` byte-identical |
| AC-F3 | `parts/src/receipt-row.mjs`: seal `glyph(sealMark[0], sealMark[1], { size: SEAL_GLYPH_SIZE, ... })` → `size` argument removed | `node --test` → `Error: a glyph is drawn at a stated size; there is no default size (received undefined)` thrown from `requireSize` in `glyph-sheet.mjs:243`, propagating up through `row`/`receiptRow`, failing 2 tests | `cmp` byte-identical |
| AC-F4 | `parts/src/glyph-sheet.mjs`: `glyph()` svg attrs, removed `...STROKE` spread (the fill/stroke reset) | `node tools/shoot.mjs` → `overlaps=0` and `oversize=0` unchanged (DOM rects stayed normal) while `filled=20` / `filled=10` / `filled=4` went nonzero on every capture (was `filled=0` at baseline) — the DOM-normal / screenshot-only-red split the AC asks for | `cmp` byte-identical |

**Honest gap on AC-F1's second clause**: AC-F1's machine form has two parts — (1) CSS confirms an opaque background, (2) "実screenshot判定" (the `tools/shoot.mjs` pixel/rect probe) goes red when the background is dropped. Only (1) was falsified above (via `parts/test/receipt-row.test.mjs`, not `faces/ledger/test/`, which does not cover this file at all). (2) was also performed — `node tools/shoot.mjs` was re-run with the same transparent-background break in place — and it stayed **green** (`overlaps=0` on all 7 captures, unchanged from baseline). This is not a fake pass: the probe's overlap reading is purely geometric (`getBoundingClientRect` intersection of text-bearing nodes), and the note stays a non-positioned block in flow either way, so its box never intersects the row's box regardless of background colour. **The negative control as written for AC-F1's screenshot clause does not turn `shoot.mjs` red** — an opaque-background regression is caught today only by the source-level style assertion in `parts/test/receipt-row.test.mjs`, not by the visual probe. This is recorded honestly rather than treated as a pass; AC-F1's negative-control cell for the screenshot clause specifically is `[ ]` pending either a stronger probe (e.g. rasterised pixel diff) or an accepted reliance on the CSS-level test alone.

**Final green totals after all four breaks restored**:
- `node --test "test/*.test.mjs"` (faces/ledger): **61 pass / 0 fail / exit 0**
- `node tools/shoot.mjs`: all 7 captures — `overlaps=0, repeatedRows=0, oversizeGlyphs=0, filledGlyphs=0, clippedWithoutFull=0, sprites=1` (all-zero, matches baseline)
- `node --test "membrane/test/discipline.test.mjs"`: **17 pass / 0 fail / exit 0**

**Nothing was impossible to perform** — AC-F1 through AC-F4 as written in `req/03 §4-3` were all executable with the infra present in this repo (`node --test`, `tools/shoot.mjs`, real Chrome via `tools/rig/renderer.mjs`). The one honest gap is the AC-F1 screenshot-clause non-detection noted above, which is a property of the current probe rather than an infra absence.

## Seen and not seen (denominator)

- The retired tree's ledger face: **0 lines**, by construction. Its `FACE.json` was not opened either — the consumed-method list in `req/03 §3-1` was read from the requirement document, and the names used here are the membrane's derived ones, which no other implementation could have supplied.
- `gx-api` handlers: **0 lines**. The five members a row is read from are what this face **looked for**, not what the server is known to send; anything absent is drawn as a declared hole naming the member.
- `glovrex_web/req/phase/app_ledger.req.md`: **0 lines**.
- Real `gx serve`: **0 calls** from this face.
