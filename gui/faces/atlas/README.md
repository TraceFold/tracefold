# faces/atlas — **what subjects has this window read, and what was last said about each** (F-6, LAST face)

> **status**: implementation landed 2026-08-25, from zero; retrofitted the same day for Owner directive #340 (see the last section, which supersedes items 3 and 4 below). `node --test "test/*.test.mjs"` = **102 pass / 0 fail / exit 0** (81 before the #340 round). Whole-tree suite (`membrane`+`shell`+`parts`+`faces/*`) = **665 pass / 0 fail / exit 0** (baseline before this lane: 584). Real-renderer captures = **7** (3 fixture states x narrow/dark, `atlas` additionally x wide) + browser-mount-smoke + 2 real-window (light/dark), all read visually. **This is the sixth and last face this app builds (5/6 -> 6/6)**.
> **origin**: written from `req/03_FACES_REBUILD.md §2 F-6` (the one-line identity: "主面。stage col分割の主葉に置く「見る面」(drill-down起点)"), `req/08_MAIN_SURFACE_SEMANTICS_AUDIT.md §2` (default-slot placement ruling, Owner ruling 263) and `§14` (document/graph/timeline interaction-model ruling). **COPY HARD BAN kept structurally**: `ui_proto` was never opened for this face -- and could not have been, since `ui_proto` never had an atlas/main face at all (see "no reference row" below).

## Owner eye-judgment corrections, applied from zero

A direct Owner eye-judgment pass over the five already-built faces (`faces/ledger`, `faces/held`, `faces/receipt`, `faces/graph`, `faces/notice`) found five defects. This lane's brief was corrected to fold all five into `faces/atlas` from the start; the retrofit of the five earlier faces is a separate, later lane's scope, not this one's:

1. **Detail panels collapsed by default.** Every subject's own touch history is a native `<details>` constructed CLOSED unless `needsOpen()` finds a genuine reason to start it open (a hole on the latest touch, or a verdict word long enough to overrun `parts/src/verdict-badge.mjs`'s own clamp). Three dedicated tests (`test/atlas.test.mjs`) prove both directions: an ordinary subject stays closed, and a genuinely defective one is forced open with its full value still findable on the page.
2. **Bordered controls, one row, self-evident labels.** `why`/`legend` are drawn as bordered, compact `<details>` controls (`controlToggle()`) sitting side by side inside one `data-role="control-row"` flex container, each carrying a 2-3 word plain-language hint next to its label ("why -- about this screen", "legend -- symbols used") rather than a bare word.
3. **Density.** *(superseded in part by "Owner directive #340" below: the subject and its standing moved into a box head above this line, and the line lost three of its cells. What is written here was true of the first build and is kept so the change is readable.)* Every distinct path this window read gets exactly one compact summary line (fold glyph, subject glyph, path, touch count, latest verdict, latest effect, latest time) -- not a full delta row per touch. A label that would otherwise repeat per row (the "not a receipt" style caveat, the order rationale) lives once in the legend or the claims section's own aside, not inline on every subject.
4. **Compact header + no face-switcher.** *(superseded in part by "Owner directive #340" below: the two denominators are now the first two figures of the stat band under the header, not a clause inside it. The header states the face name and the declared question.)* A single `data-role="face-header"` line states the face name and both denominators (`N subjects, M touches read`) before anything else is drawn. A face-switcher is **not** built here: `glovrex/req/08 §2-1` already rejected the "hub" shape by name (a main face that names other faces breaks req/03 §1's zero-other-face-import line from the face side), so this is a structural non-goal, not a cost-based skip -- see declaration.mjs's `UNDRAWN` entry and the correction's own escape valve ("if not cheap, note as non-goal").
5. **No background bleed at capture edges.** Traced to `shell/tools/real_window.ps1` itself (a shared instrument every face's own real-window capture uses): `GetWindowRect` includes several pixels of invisible DWM resize border past the window's real visible content, and `CopyFromScreen` against that rect copies a sliver of whatever sits behind the window along with it. Fixed in place (patched, not duplicated) with `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`, `GetWindowRect` kept as a fallback. Evidence the fix actually ran on this lane's own captures: `record/real-window.json`'s `capturedRect` (`47,40 1426x893`) is narrower than the requested `1440x900` window -- the legacy path would report a rect at or larger than the request, never smaller.

## No reference row for this face

Every other face's declaration states an honest deviation from (or exact match to) a reference method/mark count in `req/03 §3-1`. This face has no such row to compare against: `ui_proto`'s `FACE.json` set never had an atlas/main face at all (`glovrex/req/38 §510`/`§518`: the "hub" shape was considered and rejected for the *reference* tree's own successor design, not only for this rebuild). `declaration.mjs`'s own test states this directly rather than silently reporting a deviation from nothing.

## The one discipline this face exists to hold

req/03 §2's one-line identity for this face -- "主面。stage col分割の主葉に置く「見る面」(drill-down起点)" -- and `req/101 §6`'s attest/render warning (named there for MODO/JIN, applied here to this face's own decision) are both held structurally, not by comment alone:

1. **Every distinct path becomes a subject, not only the ones touched twice or more.** That filter is `faces/graph`'s different question (F-4); a path read exactly once still gets a summary line here (`test/atlas.test.mjs`: "a path touched exactly once IS drawn as a subject here (unlike faces/graph, which would not draw it at all)").
2. **No chain edge is ever drawn.** `structure/child`/`structure/outside`/`childOf` are `faces/graph`'s vocabulary; this screen states a count and the latest touch's own facts, never a resolved link between two touches (`test/atlas.test.mjs`: "this face draws no structure/child or structure/outside mark anywhere").
3. **A subject's grouping and "latest touch" decision is computed once, in `toRecord()`/`buildAtlas()`, and read as already-decided data everywhere under `view()`.** Every subject/touch object is frozen at construction; `needsOpen()` reads that frozen object, never a live re-derivation.

**Negative control performed, not just planted**: the shipped `atlas.mjs` was hashed (`sha256 fc4e5b3ad0221df18153b7a5f5198b71b0a29945892170f90cff46e70353cd38`, 28,037 bytes) and backed up before a real bug was introduced -- the computed decision `const open = needsOpen(subject);` was changed, for real, to the hardcoded literal `const open = true;`. Against the shipped fixture, **four independently-computed detectors fired red**: `tools/gate.mjs`'s `no-hardcoded-subject-open` source check (fired on the actual planted line, `atlas.mjs: const open = true;`, not an isolated string) and three separate behavioural assertions in `test/atlas.test.mjs` ("a normal subject...is constructed CLOSED by default", "density: ...no subject...should be forced open" -- caught 3 wrongly-forced-open subjects instead of 0, and "needsOpen: a hole on the latest touch forces that subject open, and only that one" -- caught an unrelated subject wrongly forced open). The file was restored from the byte-for-byte backup; `sha256` of the restored file matched the pre-bug hash exactly (`cmp` confirmed byte-identical), and a second `node --test` pass confirmed green again (81/81), followed by a whole-tree pass (665/665).

## `default_slot`/`emits`/`handles`: declared, not wired

`glovrex/req/08 §2-3`/`§2-4` describe a slot/address-resolution shape so a shell can find the primary face and route a drill-down address without ever holding a face-id literal. This face declares `default_slot: "primary"`, `emits: []`, `handles: []`, each with its own honest-gap reason string (`declaration.mjs`'s `SLOT_WIRING`/`EMITS_REASON`/`HANDLES_REASON`) -- no shell code in this tree reads any of the three fields; wiring that resolution is a later, separate lane's scope, the same boundary `req/100 §7` already draws around `faces/graph`'s own RC-5. `test/declaration.test.mjs` checks both premises directly rather than assuming them: no other face in this tree also declares `default_slot: "primary"` (app req/08 AC-S1's "exactly one" rule would already be broken if two did), and no other face declares an `emits` field at all today (so `HANDLES_REASON`'s "nothing to handle yet" claim is checked, not assumed).

## Real-renderer defect found and fixed, not only checked

`tools/shoot.mjs`'s first run against this face's own fixture found a genuine `clippedWithoutFull` defect (5 cells on the narrow capture) that no unit test had caught: the subject summary line's `path` cell (ellipsis-clipped, mono font, `minmax(0,14rem)`) was clipping even ordinary short paths like `/work/report.md` in the real renderer, and because a closed subject's own detail is hidden, the full path never appeared a second time anywhere visible on the page -- a real N-4-class defect on a screen this lane wrote specifically to avoid the class of bug the five earlier faces' own Owner eye-judgment pass just found. Root cause: the `Math.floor(rem*2)` character-budget convention `req/100 §1` states elsewhere does not hold for a flexible mono-font column at this size (measured, not assumed, by the renderer). Fixed by construction, not by tuning the budget further: the `path` cell now wraps (`overflow-wrap: anywhere`) instead of clipping, so it cannot lose data regardless of length; the `at` cell now shows a deliberate, complete, non-overflowing date+hour substring (the same truncation convention `req/100 §1` already states for the shared row grid's own `at` column) instead of an ellipsis-clipped full ISO string. Re-shot: `clippedWithoutFull=0` on every one of the 7 captures.

## The shape

| file | what it is |
|---|---|
| `declaration.mjs` | the only place a server method is spelled out; consumes (1, no reference to compare against) / acts (0) / marks (10: 9 reused + `structure/subject` new) / `default_slot`/`emits`/`handles` (declared, not wired) / undrawn / tests |
| `binding.mjs` | the seam to the parts -- row/glyph/badge/order/checkable, no `provenance-fold` (no settled/held pair) and no `seal-claim`/`serial` (no seal claim of any kind on this screen) |
| `atlas.mjs` | `createFace({parts})` -> `{ mount, read, view, toRecord, callerFor }`. `toRecord()`/`buildAtlas()` are the attest step (group by path, resolve each subject's `latest`/`earliest` touch once); `view()` and its section builders are the render step, a pure function of what attest already produced |
| `gate.mjs` | 18 machine checks: 13 over shipped source (10 shared + `no-hardcoded-subject-open` + `no-scrolling-container` + `no-inline-cursor`), 5 over what was drawn (4 shared + `fold-mark-agrees-with-open-state`). `node faces/atlas/tools/gate.mjs` now runs them and prints them -- until the #340 round the file had no runner at all and that command exited 0 having checked nothing |
| `index.mjs` | the one door a shell mounts |
| `tools/fixture.mjs` | three states (`atlas` / `atlas-empty` / `atlas-unread`) written as pages a browser can draw; `atlas` includes one pathologically long path so the forced-open case is seen actually happening, not only declared |
| `tools/shoot.mjs` | the same pages in front of a real renderer, photographed and measured (adds `subjectsDrawn`, `subjectsOpen`) |
| `tools/browser-mount-smoke.mjs` | real-browser (headless-renderer) mount smoke, the W15 clause-2/3 evidence |
| `tools/real-window-smoke.mjs` | drives `shell/tools/real_window.ps1` (this lane's DWM-extended-frame-bounds fix included) in both themes |
| `tools/bench.mjs` | `view(state)` over 1,000 independent synthetic states of 60 transformations (20 paths x 3 touches) each |
| `tools/interaction-pass.mjs` | G-7 round (below): presses all four control disclosures and one subject's own fold in a real window, a screenshot after each act, `sha256`-checked against itself for byte-identical (no-op) captures |
| `test/declaration.test.mjs`, `test/atlas.test.mjs`, `test/gate.test.mjs` | 81 tests total; `stub-port.mjs`/`dom-stand-in.mjs` imported directly from `faces/ledger/test/`, the same precedent every other face in this tree already set (req/99 §5) |

## Placement and semantics, written before code

`req/100_PLACEMENT_SPEC.md §8` and `req/99_SEMANTICS_REGISTRY.md`'s additive drift block for this face were both written before `atlas.mjs` existed, per this lane's own brief. §8 states each Owner-correction-driven placement decision and why; req/99 carries all 15 in-scope files with the same additive-not-renumbering discipline every prior drift block already set.

## Five-principles checklist (`INHERITED_PRINCIPLES.md §3c`)

1. **template-form** -- PASS, with one honest exception stated, not hidden: the always-visible subject summary line is a bespoke grid, not `req/100 §1`'s 8-column `receipt-row.mjs` grid, because a folded subject is not a delta and forcing it into that grid would misuse cells whose meaning does not transfer (e.g. `seal`). The per-touch *detail* underneath each subject reuses `receipt-row.mjs`'s `row()`/`note()` unchanged -- those are real delta records, the same meaning every other face already draws them with.
2. **lightweight+bench** -- PASS with a figure: `node tools/bench.mjs` -> **median 5.180ms** for `view(state)` over 1,000 states of 60 transformations each (budget 300ms). Mount ms and real paint remain unmeasured, the same open axis every other face states. *(Re-measured after the #348/#349 round: **median 7.0ms**, same budget -- see the last section of this file. The figure above is kept as the reading it was rather than overwritten.)*
3. **english+comments** -- PASS. All source English; `no-borrowed-symbol`/ASCII-only gates hold at zero on shipped source, including a real near-miss caught and fixed during this lane (an ellipsis character `...` was written as the Unicode `…` glyph in a first draft of `dateHourOf()`, caught by re-reading the source before it reached a test run, replaced with three ASCII periods).
4. **always-CRUD** -- read (R) is the whole of what this face does, by design, and states so structurally (`ACTS = Object.freeze([])`) rather than by omission.
5. **DB-principle** -- PASS. No store: every value is read fresh from the port on each `read()` (`caller.fold(READS.transformations)`); fixtures are declared-rebuildable (`node tools/fixture.mjs`), never hand-edited.

## `[ ]` -- not done, and not called done

- `[ ]` **No independent re-run.** Every number in this README is this lane's own first pass. `req/03 §2`'s `[●]` requires a second, independent session's re-verification, the same discipline every other face in this tree still carries as `[◐]`.
- `[ ]` **RC-5 (no control on this screen reaches another face) is open here too**, and this face additionally cannot be reached *from* another face (no row on `faces/ledger`/`faces/held`/`faces/receipt`/`faces/graph` links here, and this face draws no address any other face could follow to it either -- `emits`/`handles` are both declared empty).
- `[ ]` **The document/graph/timeline lens-switching machinery (`glovrex/req/08 §14`) is not implemented.** This face implements the document model only, consistent with §14's own risk analysis (kill-battery G-1..G-5) recommending document as the safest default when the read population is layer-A-thin, which is this repo's actual state today. Which of `act`-extended or a new namespace a future lens-mode toggle would use is not decided here.
- `[ ]` **Shell-level `default_slot`/`emits`/`handles` wiring does not exist.** No `shell/kernel` code resolves these fields into an actual mounted position or address routing; this lane declares the data, not the mechanism.
- `[ ]` **No interaction pass (req/38 SS24k)** was performed for this face -- this screen's interactive controls (two disclosures, every subject's own fold) were not individually pressed and screenshotted after each act. *(Partly superseded by the #348/#349 round, last section of this file: the right-click menu now has a real-window interaction pass with ten measured properties. The two disclosures and the subject folds are still not individually pressed, so this line stays `[ ]`.)* *(Further superseded by the G-7 round, last section of this file: `tools/interaction-pass.mjs` now presses all four control disclosures -- this screen grew two more since the #348/#349 round, `claims` and `omitted`, that line above never counted -- and a subject's own fold, independently, with a screenshot after each act. This line stays `[ ]` only for the part still true: no independent re-run of any of this file's own numbers has happened, on this evidence or any other.)*
- `[ ]` **Narrow-width real-window capture is absent.** `real_window.ps1` opens at a fixed 1440x900; `tools/shoot.mjs`'s headless narrow (720px) and wide (1280px) captures are the only other-viewport evidence.
- `[ ]` **`get_transformations`'s pagination has never reached a real `gx serve`.** The same open item every other face's README states for its own routes.
- `[ ]` **A destructive fault-injection sweep across every shared AC (AC-F1..AC-F4 style) was not repeated here.** The genuine negative control performed (above) targeted this face's own defining property (open/closed state); the four generic negative-truth-ledger ACs are inherited unmodified from the shared parts every other face already fault-injected.
- `[ ]` **The real_window.ps1 capture-bleed fix (this lane) has not been independently re-verified against a background window deliberately placed behind the capture target.** The evidence in `record/real-window.json` is a measured-rect argument (captured rect narrower than requested window size) plus a visual read of two captures that happened not to have another window directly behind them at capture time -- a deliberate adversarial repro (a second window placed to overlap the capture region) was not performed.
- `[ ]` **The five earlier faces' own Owner-correction retrofit is out of this lane's scope entirely.** `faces/ledger`, `faces/held`, `faces/receipt`, `faces/graph`, `faces/notice` still draw expanded-by-default detail panels and stacked full-width `why`/`legend` bands; this README does not claim otherwise.

## Seen and not seen (denominator)

- The retired tree: **0 lines**, by construction -- and could not have been more than that, since `ui_proto` never had an atlas/main face at all (see "no reference row" above).
- `gx-api` handlers: **0 lines**. The members this face looks for (`at`/`actor`/`effect`/`verdict`/`path`/`digest`/`sequence` on each transformation) are what this face looked for, not what the server is known to send -- `membrane/wire-fields.json`'s own hole statement confirms this domain contributes no fields yet.
- `glovrex_web/req/phase/app_atlas.req.md`: **0 lines** (not located; not opened either way).
- Real `gx serve`: **0 calls** from this face.
- `glovrex/req/08_MAIN_SURFACE_SEMANTICS_AUDIT.md`: read in relevant part (§2 default-slot ruling, §2-3/§2-4 declaration shape, §14 interaction-model ruling) -- its full M0-M7 semantics-extraction pipeline (§7) and its AC-S1..AC-S27 machine-gate list (§4) were read but **not implemented**; this face declares the slot/address shape §2-3/§2-4 describe and stops there, stated as a non-goal rather than silently partial.

## Owner directive #340 -- the band, the boxes, the measured footer (retrofit round, 2026-08-25)

Owner #340's reading of this tree: monotone, hard to grasp at a glance, hard to operate. What
changed here, each with the number that says whether it changed:

1. **A stat band under the header, five figures.** `subjects` / `changes` / `Admit` / `Deny` /
   `Escalate`. The two denominators moved out of the header's prose into the first two columns,
   so `ROWS.reports_denominator` is now discharged in figures; the three standings carry their own
   hue and their own mark. A count this screen cannot know draws a dash, never a zero -- the
   refused fixture draws five dashes and the empty one draws five zeros, and those are two
   different pictures on purpose. The three standings plus the ones that could not be placed always
   sum to the subject figure; a subject whose latest change carries a word the sheet does not hold
   is counted nowhere and says so, with a gap mark on the subject figure itself.
2. **Every subject is a box.** `parts/src/surface.mjs` `box()`, head carrying the subject, its own
   count (`3 changes`, `1 change`) and the standing of its most recent change as a filled pill.
   The fold line underneath lost the three cells the head now carries (path, count, verdict) and
   keeps the two it does not (effect, time). Cost, measured in the shot: a shut subject went from
   **37px to 81px**, so this screen fits fewer subjects per window than it did -- roughly 8 in a
   900px window against roughly 20 before. That is the price of the Box idiom at this box's fixed
   10px margin and its head's own height, and it is stated rather than hidden.
3. **The per-subject sentence is gone.** `every touch this window read for <path>, oldest to
   newest` was drawn once per subject (3 times on the fixture); the box head names the subject and
   the legend states the order once. Visible text on the answered narrow capture: **880 -> 872
   characters** with more facts in it.
4. **No scrolling container, checked at the source.** New gate `no-scrolling-container` (with a
   negative control, and a positive control proving it does not fire on `overflow:hidden` or
   `overflow-wrap`, both of which this face uses on purpose). Grep of this face's own shipped
   source for `overflow` / `max-height` / a fixed `height:`: **0 scrollers, 0 height bounds**;
   the only `overflow` here is `hidden` on the fold row, which clips and does not scroll.
5. **A measured runtime footer.** `render` is `performance.now()` around the whole tree build
   (attest step included, footer and frame excluded because neither can be built until the number
   exists), so the figure on the shot is that draw's own: **15.5ms** on the answered narrow
   capture, **0.6ms** on the refused one. `read` names what this face read in its own words, and
   is a dash on a screen that read nothing rather than a source it never had.

**Panel-open state across a repaint (req/103's finding, checked here): n/a, and this is the
mechanism.** `mount()` paints exactly twice -- the waiting screen synchronously, and the drawn
screen once, when the single read settles -- and holds no other route to `paint()`. This face
declares no act, subscribes to nothing and starts no timer, so nothing can ask it to draw again;
the open state of every disclosure lives on a `<details>` element that is never rebuilt under the
reader. Pinned by a test that writes into the host after the read has settled and finds it still
there.

**Defects found in this face's own already-shipped code during the round** (guilty presumption,
all four fixed): (a) `tools/gate.mjs` had no runner -- `node faces/atlas/tools/gate.mjs` imported
the module, checked nothing, printed nothing and exited 0, and the checks were reachable only from
the test file; (b) every control on this screen declared `cursor:default` inline, which outranks
the shared rule set's `cursor:pointer` on a summary, so the three things a reader is meant to press
were the three saying they could not be (new gate `no-inline-cursor` holds it); (c) the refused and
absent screens drew `code: UNAUTHORIZED` and the raw JSON of the route that was asked for -- the
two shapes `req/96` axis B scores zero for, on a face that had already removed them from every
other part of itself; (d) the test that should have caught (c) could not: it read the screen through
`textOf()`, which concatenates text nodes with nothing between them, so `UNAUTHORIZED` immediately
followed by the footer's `render` had no word boundary and matched no pattern. The reading now
joins with a space and runs over all five read outcomes, not only the one that answers.

**Left behind, not fixed here** (both belong to `parts/`, which this lane may not edit): the box
head lets its standing pill shrink before its name does, so the subject with the pathologically
long path draws its pill clipped to `Ad...` (the full word is in the note underneath, and
`clippedWithoutFull` reads 0, but it is ugly and the fix is a `flex` rule in `box()`); and `box()`'s
fixed 10px bottom margin plus its head height is what sets the 81px-per-subject figure above, so a
compact variant of `box()` is where this screen's density would come back.

**Measured after the round** (all from the commands in the lane brief, run from the repo root):
`node --test "faces/atlas/test/*.test.mjs"` = 102 pass / 0 fail; `tools/gate.mjs` = 18 checks, 0
fell, exit 0; `tools/bench.mjs` median 6.5ms against a 300ms budget; `tools/shoot.mjs` = 7 captures,
overlaps 0, repeated rows 0, oversize glyphs 0, clippedWithoutFull 0, under-tap-budget controls 0,
horizontal overflow 0.

## Owner directives #348 and #349 -- the icon floor, the right-click, the words, the shared shapes (retrofit round 2, 2026-08-25)

**1. The icon floor (#348 (3)).** Eleven call sites in `atlas.mjs` said `size: 14` -- more than
any other face in this tree, and the largest single share of `parts/test/glyph-sheet.test.mjs`'s
failing floor gate (27 sites across six faces). They now say `size: MARK`, and `MARK` is
`binding.mjs`'s `minReadable`, which is `parts/src/glyph-sheet.mjs`'s own `MIN_READABLE` -- so this
face and the gate that judges it cannot hold two numbers. `minAct` (20) is reached and deliberately
not used: it is the floor for a mark on a control that *sends*, and this face declares no acts, so
nothing here is entitled to it (the shell draws its own tab marks at `minReadable` for the same
reason). The floor gate is green: **11 -> 0** sites named on this face, `17/17` in
`parts/test/glyph-sheet.test.mjs`.

The cost, measured rather than assumed: **nothing moved**. A shut subject is 81px before and 81px
after, because the fold line's height is `--row` (36px) and the box head's is a 20px line box plus
10px of padding -- both already taller than a 16px mark. `oversizeGlyphs` 0, `clippedCells` 1 (the
same declared time cut as before), `horizontalOverflow` 0, `underTapBudget` 0 on all seven captures.
A second reading was added where the source grep cannot reach: `tools/gate.mjs`
`marks-are-at-or-above-the-floor` reads the width that actually reached the tree across all three
states (a name can be bound to anything), and `tools/browser-mount-smoke.mjs` reads the width that
reached a real window. Both were fired red first, on a planted 14px mark.

**2. The right-click (#348 (2)).** This face declares `ACTS = []`, so it is the "copy value" case:
there is no act to offer here, and rather than skipping the atom the menu **says so**, in a disabled
line carrying `declaration.mjs`'s new `ACTS_REASON`. A reader who right-clicks a row on a face that
acts and then right-clicks one here is owed the difference in words; the browser's own page menu
tells them nothing. What the menu offers comes from `declaration.mjs` `OFFERS`, mapped, never named
inline -- `test/atlas.test.mjs` holds that every entry id is one the declaration knows.

Every cell drawn as a value carries `data-menu-value` and every fold line carries its own subject,
so a right-click anywhere on a subject has something to take; a cell drawn as a stated gap gets the
same entry disabled, with the offer's declared reason. The value taken is the **full** one: the
time cell draws a declared cut (`2026-08-24T09...`) and the menu copies `2026-08-24T09:07:00Z`,
which is the one thing this menu can do that no column width can.

No `position` is written for it, and that is not a way around this face's own `nothing-out-of-flow`
gate -- it is why the gate can stay at zero and the menu can still escape the `overflow:hidden`
every row cell declares. `popover` puts it in the browser's top layer and the two coordinates that
place it at the pointer are set on the node when it opens. It also buys three of the properties this
atom asks to be pinned from the platform rather than from a handler. All three are pinned in a real
window (`tools/browser-mount-smoke.mjs`, real `MouseEvent`s, real top layer): `escapeClosed` true,
`clickAwayClosed` true, `menusAfterSecond` **1**, `inTopLayer` true, `defaultRefused` true. A repaint
takes the menu down (pinned in `test/atlas.test.mjs` against the waiting-to-answered repaint) and
unmount removes both the node and the listeners.

`copy value` uses the clipboard and states which way it went, the same shape `shell/kernel`'s own
`command-copy` holds: `data-copied` / `data-copy-failed` on the menu, **and** a line drawn in it, so
it is not only in an attribute. In the headless window it reads `copied: false` and draws "this
window would not let that be copied" -- a synthetic event carries no user activation, the write is
refused, and the menu says so rather than looking identical either way. That is the atom working,
not the atom failing.

**3. The words (#348 (4)).** Weight is mechanical now: one `TYPE` table (head 700, label 500, body
400), one `typed()` builder, and a gate (`weights-come-from-the-scale`) that refuses a weight
written by hand -- the file had `'600'` at two sites, `'700'` at one and **nothing at all at eleven
others**, so most of the screen's weight was whatever the document inherited. There is deliberately
no `figure` tier here: every number on this screen is drawn by the shared band and box head, which
carry `.gx-figure` (600). `tools/shoot.mjs` reads the weights that reached the page: **400 / 500 /
700** from this face, 600 from `parts`.

Breaking: `overflow-wrap: anywhere` was on five text styles -- an instruction to break inside a word
at any letter -- and is now `break-word` plus `text-wrap: pretty`, kept as `anywhere` only on the
two cells that hold a machine value with no spaces in it. Measured, not asserted: `tools/shoot.mjs`
now finds every line of every visible run of text by asking the engine where each character landed,
and counts breaks with a letter on both sides and lines of one character. **midWord 0, orphan 0** on
all seven captures. The reading was fired first, at 300px, where it returns 1 and 10 -- and both are
in `parts/`, reported below.

Redundant words: **-442 characters** of drawn text across the three states (measured by rendering
this face's tree at `6e2695d` and at this commit and differencing, with the two things this round
adds for other reasons -- the actor column and the legend entry for the menu, +603 -- subtracted
out). What went: two `<h2>`s that were the name of the control they were drawn inside
(`claims -- what you can check` over "what you can check here yourself"); three control hints that
were their own label in more letters (`legend -- symbols used`, `omitted -- what is not drawn`,
`why -- about this screen`); five legend names that said where the thing is while the reader is
looking at it (`the time shown on a subject line` -> `the time`); `changes this screen could not
file under any subject` -> `changes filed under no subject`; and `not yet read` -> `not read`, where
`yet` promised a later reading that two of the three states it is drawn on are not waiting for.
A wire noun went with them: `the list of transformations could not be read` was the route's word for
a thing this screen calls a **change** everywhere else, on the one state a reader only reaches when
something has already gone wrong. The rule is a test now rather than a list of literals: a control
hint may not share a word with the name beside it, and no `<h2>` may exist on this screen at all.

One candidate was looked at and **put back**: `outcome: refused` above `forbidden: this token may
not list transformations` reads like one fact twice, and removing it cost three fail-closed tests
the property they exist to hold -- the four outcomes are four different ways to have no list, and a
transport failure's own sentence carries no word for which one it was. Sixteen characters is not
worth a reader being unable to tell a refusal from a broken connection.

**4. The shared shapes (#349 (3)).** Inside this face: `heading`/`aside`/`plain` were three copies
of one `el(tag, { style: style({ ...the same six declarations... }) })` block that disagreed about
weight by omission -- they are three `typed()` calls now, and `heading()` is gone entirely because
both of its callers were the redundancy above. The `kvLine` grid and `notDrawnSection`'s omission
row were the same two-column grid with a different first column width; the width is an argument now.
Three ternaries asking "is this member a gap, or is the whole change missing" are one `gapFor()`.
The cross-face duplications this lane can see are in the lane report for the seat, not here.

**5. Pinned mechanically (#348 (5)).** Three new source rules in `tools/gate.mjs`, built to the
shape `no-inline-cursor` and `no-scrolling-container` already had rather than a second one:
`no-raw-motion` (no `transition:` and no bare `ms` -- the motion route owns both), `no-raw-corner`
(a corner is taken from the scale by name) and `weights-come-from-the-scale`. Each was fired red
before it was believed. `no-raw-corner` fired on **shipped code**: `controlToggle()` was carrying
`'border-radius': '4px'`, tier 1's number on a control, picked by eye before the scale existed.

And a lesson about grep rules that is worth more than the three rules: written as
`:\s*(?!T\.radius)` the exemption **backtracks to zero**, the lookahead is tested against a space,
and the rule fires on the code it exists to require. Two of the three did exactly that on first run,
on the lines this round had just fixed. Written as `:(?!\s*T\.radius)` they do not. There are now
four tests holding the exempted spelling green, because a rule that cannot tell right from wrong is
worse than no rule.

**A defect in this face the round found by looking**: every fold line was blank from about 40%
across -- three cells packed into the left half and a flexible fourth column holding a short value.
The actor is read off every touch (`TOUCH_MEMBERS`), was drawn nowhere until a subject was opened,
and is the fact a reader of a change list wants after "what" and before "when". It takes the
flexible column; the time moved to the right edge, where a column of timestamps lines up down the
screen. Visible characters on the answered screen: **872 -> 925**, of which +48 is the actor and +5
is the reworded hints.

**Left behind, in `parts/`, with numbers** (this lane may not edit them):

- `box()`'s head lets the standing pill shrink before the name. Measured at 720px on the
  long-path subject: the name is drawn **478px of the 543px** it wants, and the pill is drawn
  **30px of the 37px** it wants, so `Admit` reaches the screen as `Ad...`. Both are flex items with
  `overflow:hidden`, so both have `min-width:auto` resolved to 0 and both shrink; the name has an
  ellipsis and can afford to, and the pill is a five-letter word that cannot. `flex: none` on the
  pill (or on `box()`'s `pill` slot) is the fix. `clippedHeadParts` in
  `fixtures/shots/measurements.json` reads `name,pill` at 720px and `none` at 1280px.
- `statBand`'s `--` (`STAT_DASH`) breaks after the first hyphen. At 300px the unread screen draws
  **10** single-character lines, two per segment, because `.gx-figure` declares no
  `white-space: nowrap` and a hyphen is a break opportunity. Not visible at 720px or above.
- `receipt-row.mjs`'s note-line value breaks mid-word: at 300px `agent:packer` is drawn as
  `a` + `gent:packer`. **1** occurrence, in `[data-role="note-line"] > SPAN`.

**Measured after this round** (from the repo root): `node --test "faces/atlas/test/*.test.mjs"` =
**121 pass / 0 fail** (102 before); `tools/gate.mjs` = **22 checks, 0 fell**, exit 0 (18 before);
`tools/bench.mjs` median **7.0ms** against a 300ms budget (samples 4.3-26.8ms; the tail is the first call, before the tree builder is warm); `tools/shoot.mjs` = 7 captures, overlaps
0, repeated rows 0, oversize glyphs 0, clippedWithoutFull 0, under-tap-budget 0, horizontal overflow
0, midWord 0, orphan 0; `tools/browser-mount-smoke.mjs` = mounted, 263 elements, 0 marks under the
floor, 10 of 10 menu properties held in a real window;
`node --test "parts/test/glyph-sheet.test.mjs"` = **17 pass / 0 fail**.

**Still `[ ]` after this round**: the interaction pass above covers the menu and nothing else -- the
two disclosures and every subject's own fold were still not individually pressed and shot; no
independent re-run of any number here; and the 81px-per-subject density is unchanged, so this screen
still fits about 8 subjects in a 720px window where the pre-box version fitted about 20.

## G-7 round -- the two disclosures the menu round left, and a subject's own fold (2026-08-31)

Source: `glovrex/req/964` §2 (`G-7 | glovrex_app/faces/ | ready(F-6 atlas面の追加)`) and `glovrex/req/974`
§D-1 (`G-7 ... brief: F-6 atlas面の追加(既存部分稼働の拡張)`). Write-target for this atom is
`glovrex_app/faces/` only; `glovrex_app/shell/` is G-8's own write-target and was not opened for
edit here (read-only, to confirm `[data-role="subject-summary"]` etc. before writing the pass). No
line of `atlas.mjs`, `binding.mjs`, `declaration.mjs`, or `index.mjs` changed in this round --
everything below is a new file (`tools/interaction-pass.mjs`) plus its own generated evidence
(`record/atlas_act_0..8_*.png`, `record/interaction-pass.json`).

**What this closes**: the `[ ]` line above ("no interaction pass ... the two disclosures and the
subject folds are still not individually pressed") was already stale in one more way before this
round touched it -- this screen carries **four** control disclosures today (`why`/`legend`/`claims`/
`omitted`), not the two that line and the #348/#349 section above were written against; `claims` and
`omitted` were both already shipped and neither had ever been pressed in a real window either.
`tools/interaction-pass.mjs` presses all four, independently, and separately presses one subject's
own fold open and shut and a second subject's fold besides -- the region `subjectBox()` builds, which
no earlier pass on this face (menu included) ever pressed.

**Two real defects found while building the pass, both in the new tool, neither in shipped source**:

1. **The fixture's own entry script leaves a menu open on load.** `browser-mount-smoke.mjs`'s
   `ENTRY_SOURCE` runs its own `menuPass()` unconditionally as part of mount and, by that file's own
   design ("the last one is left open on purpose, so the shot has a menu in it"), leaves a
   `copy value` menu open over the second subject row before this pass had clicked anything at all.
   Found by looking at the first act 0 screenshot rather than trusting it -- the same discipline
   `record/interaction-pass.json`'s own `shotIntegrity` block (below) exists to hold mechanically.
   Fixed by dispatching a real click-away (`PointerEvent('pointerdown')` on `document.body`, this
   screen's own documented dismiss route) before treating the page as "initial"; `menuOpenAtStart`
   in the report is `false`, checked rather than assumed.
2. **A no-op capture trap, walked into once before it was avoided.** `tools/rig/renderer.mjs`'s
   `capture()` clips a fixed rect at the page's own origin -- `faces/receipt/tools/interaction-pass.mjs`
   already states half of this in its own comments ("anything below the first 900px is not in a shot")
   and the other half too ("scrolling first makes it worse, not better ... the shot comes back blank
   cream"), which a first version of this file read past and then reproduced anyway: it scrolled the
   acted-on element into view before every capture, and three of its nine shots -- three different
   acts, three different `window.scrollY` values, three genuinely different DOM states, checked with
   `sha256` -- came back byte-identical. They were blank: opened and looked at, not just hashed. The
   fix applied is the one already sitting in the sibling file: never scroll; open the renderer at a
   fixed, tall-enough viewport (`900x2200`, this round's own `VIEWPORT` constant) so every act stays
   inside the one region the fixed clip actually captures. Re-run after the fix: the no-op trio
   collapsed to zero.

**One remaining pixel-identical pair, and why it is evidence rather than a defect**: `record/
interaction-pass.json`'s `shotIntegrity.duplicateShotGroups` still names one pair,
`atlas_act_5_why-closed.png` and `atlas_act_7_subject-closed.png`, `sha256`-identical. Read against
the acts rather than assumed: act 5 is "why closed, legend/claims/omitted open, both subjects closed"
and act 7 is "the same, after a subject was opened (act 6) and folded back shut again" -- the same
DOM state by construction, not by coincidence. Two states that a click sequence claims are equal came
back pixel-equal too, which this pair is kept as a positive round-trip reading of rather than
silently dropped from the report.

**Independence, read from a live document rather than assumed from the four calls that changed one
state each**: every one of the four controls stayed at its own open/closed state while every other
control and the subject fold were pressed (`whyStillOpen`/`legendStillOpen`/`claimsStillOpen` through
the acts, `controlsUnchangedAfterFold` at act 6); the untouched subject's own fold state never moved
while the target subject's did (`otherSubjectsUnchanged`); a second, different subject's fold opened
independently of the first, which stayed exactly as act 7 left it (`targetStillClosed` at act 8); and
the repaint witness planted at the top of the run (the same mechanism receipt's own pass plants)
survived every act (`witnessSurvivedEveryAct: true`) -- consistent with this face's own stated
mechanism (mount paints twice and never again) rather than merely not contradicting it.

**Measured this round** (from the repo root): `node --test "faces/atlas/test/*.test.mjs"` =
**122 pass / 0 fail** (121 before this round -- the one new test came from the unrelated req/822_c7
commit on 2026-08-26, not from this round); `tools/gate.mjs` = **22 checks, 0 fell** (unchanged);
`tools/browser-mount-smoke.mjs` = mounted, menu pass green (unchanged); `tools/interaction-pass.mjs` =
**9 acts, 9 screenshots**, 8 of 9 byte-distinct (the one exception explained above),
`witnessSurvivedEveryAct: true`, every independence check `true`. `tools/shoot.mjs` and
`tools/bench.mjs` were **not** re-run this round (no shipped source changed that either reads).

**Whole-tree baseline, checked and left alone**: `node --test "faces/*/test/*.test.mjs"` from the
repo root reports **688 tests, 624 pass, 64 fail** at the start and end of this round -- all 64
failures are in `faces/ledger` (`ledger` alone: 116 tests, 52 pass, 64 fail), a different face this
atom's write-target does not include and this round did not open. `faces/atlas` alone: **122/122**,
both before and after. `shell/**` carries 2 pre-existing failures of its own (a route-count mismatch,
107 tests / 105 pass), also outside this atom's write-target (`shell/` is G-8's).

**`[ ]` -- still not done, named rather than silently closed**:

- No independent re-run of this round's own numbers (the same open item every earlier round on this
  face states of itself).
- `record/interaction-pass.json`'s `readByEye.notHeld` says what it always says: the 9 new
  screenshots were written and partially eyeballed by this round's own author in cropped form (not a
  second, independent pass) -- a genuine human read-by-eye judgment, recorded as `[ ]` until one
  happens.
- The right-click menu's own interaction pass (browser-mount-smoke.mjs, #348/#349 round) was not
  repeated or extended here -- this round's own scope was the two regions that pass does not touch.
- `faces/ledger`'s 64 failures, `shell/`'s 2, and the other four faces' own missing disclosure-level
  interaction passes (only `faces/receipt` had one before this round) are named above as seen, not
  fixed -- none are this atom's write-target.
