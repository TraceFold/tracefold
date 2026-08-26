# faces/graph — **what has been touched twice or more** (F-4)

> **status**: implementation landed 2026-08-25, from zero. `node --test "test/*.test.mjs"` = **70 pass / 0 fail / exit 0**. *(Superseded on 2026-08-25 by "Retrofit round 4" at the foot of this file: 90 pass / 0 fail, and the bench figure quoted in the five-principles checklist below is restated there. The original numbers are left standing rather than edited, so the delta is visible.)* Real-renderer captures = **7** (3 fixture states x narrow/dark, `graph` additionally x wide) + browser-mount-smoke + 2 real-window (light/dark), all read visually.
> **origin**: written from `req/03_FACES_REBUILD.md §2 F-4`, `§3-1`'s graph row, `§5`'s graph requirement, and app `req/08 §14` (graph-route view = togglable lens ruling, Owner #263). **COPY HARD BAN kept structurally**: `ui_proto/ui/faces/graph/*` was never opened -- not the module, not its FACE.json, not its tests. The only reference-tree facts consumed were the contract row in req/03 §3-1 (2 declared methods, 11 declared marks) and the one-sentence requirement in §5, both read as requirement documents, not code.

## The one discipline this face exists to hold

req/03 F-4 (§5): "同じ行の第2の投影。両端が見えている物だけ描き、外へ出る線は「描いていない」と言う。絵に到達しなかった行を必ず数えて出す" -- the second projection of a row this app already draws once, on `faces/ledger`: the same `path` recurring across more than one transformation. Three facts follow directly and are held structurally, not by a comment alone:

1. A path read exactly once is not a graph subject. The question is "twice or more"; a once-touched path contributes nothing to the picture, and the count of such paths is stated on screen every time this face draws (`notDrawn.touchedOnce`), never folded silently into the subject total.
2. An edge is drawn only when this window actually read both the touch it starts from and the touch it ends at, for the same path. A touch naming a predecessor this window did not read -- or one that turns out to belong to a different path -- is a line leaving the window, declared not drawn (`notDrawn.edgesOutside`) rather than guessed at.
3. Both counts, and every edge decision, are computed once in `toRecord()`/`buildGraph()` (the attest step) and read as already-decided data everywhere under `view()` (the render step). This is the same split `req/100 §6` states for `faces/receipt`, and the same trap `req/101 §6` names for MODO/JIN's enabled/disabled decision: nothing under `view()` re-derives "is this path a subject" or "is this edge drawn" by a second pass over the raw wire answer or a mutation of an already-attested node. Every node object is frozen; `childOf` (the field that turns a plain row into a chained one) is set once, at construction.

**Negative control performed, not just planted**: the shipped `graph.mjs` was hashed (`sha256 ae957923f3968195d7370a461b1e672b1c8f9de5626f203bc7d9e823d8549641`, 26,218 bytes) and backed up before a real bug was introduced -- the resolved-lookup assignment `childOf: predecessor.id` was changed, for real, to the hardcoded literal `childOf: 't-001'`. Against the shipped fixture, **two independently-computed detectors fired red**: `tools/gate.mjs`'s `no-hardcoded-childof` source check (fired on the actual planted line, `graph.mjs: return Object.freeze({ ...node, childOf: 't-001' });`, not an isolated string) and three separate behavioural assertions in `test/graph.test.mjs` (the a.md chained-row test, the d.md path-collision test, and the e.md chain-claim test -- each comparing a different row's `data-child-of` against the id the bug now wrongly forced). The file was restored from the byte-for-byte backup; `sha256` of the restored file matched the pre-bug hash exactly, and a second `node --test` pass confirmed green again (70/70).

## The reference declaration's method count, honestly deviated from

`req/03 §3-1`'s graph row declares two consumed methods: `transformations / subscribe`. Like `faces/held` (5 of the reference's 6) and unlike `faces/receipt` (2 of 2), this declaration matches only 1 of the reference's 2 -- `get_transformations`, verified against `membrane/route-table.json`'s live rows at declaration-test time. `subscribe` has no membrane equivalent (the same honest gap `faces/held`/`faces/ledger` already state for the identical reference word); `get_stream` is the nearest real route and is declared undrawn instead, not silently substituted in to make the count match.

Marks: **9, not the reference's 11** -- derived from what this face's binding actually draws (verdict Admit/Deny/Escalate, effect write/delete, structure child/hole/outside, undefined). One mark is new and belongs to this face alone: `structure/outside` (`parts/src/glyph-sheet.mjs`), added to the shared sheet per this lane's own convention ("if a needed glyph is missing, add it to the app glyph module"; no reuse of an existing meaning). `parts/test/glyph-sheet.test.mjs` (34/34) and `parts/test/boundary.test.mjs` were re-run after the addition and remain green.

## The shape

| file | what it is |
|---|---|
| `declaration.mjs` | the only place a server method is spelled out; consumes (1, honestly deviated from the reference's 2) / acts (0) / marks (9) / order / rows (denominators reported) / undrawn / tests |
| `binding.mjs` | the seam to the parts -- row/glyph/badge/order/checkable, no `provenance-fold` (no settled/held pair) and no `seal-claim`/`serial` (this screen draws no seal claim of any kind) |
| `graph.mjs` | `createFace({parts})` -> `{ mount, read, view, toRecord, callerFor }`. `toRecord()`/`buildGraph()` are the attest step (group by path, resolve edges against what was actually read); `view()` and its section builders are the render step, a pure function of what attest already produced |
| `gate.mjs` | 16 machine checks: 11 over shipped source (10 shared + `no-hardcoded-childof`), 5 over what was drawn (4 shared + `edge-state-is-not-contradictory`) |
| `index.mjs` | the one door a shell mounts |
| `tools/fixture.mjs` | three states (`graph` / `graph-empty` / `graph-unread`) written as pages a browser can draw; `graph` is built to exercise both a genuine in-window chain and a genuine edge leaving the window in one fixture |
| `tools/shoot.mjs` | the same pages in front of a real renderer, photographed and measured (adds `edgeOutsideAnnotations`, `chainedRows`) |
| `tools/browser-mount-smoke.mjs` | real-browser (headless-renderer) mount smoke, the W15 clause-2/3 evidence |
| `tools/real-window-smoke.mjs` | drives `shell/tools/real_window.ps1` in both themes, reusing the one real-window instrument |
| `tools/bench.mjs` | `view(state)` over 1,000 independent synthetic states of 60 transformations (20 paths x 3 touches) each, median of 5, `.bench/report.json` |
| `test/declaration.test.mjs`, `test/graph.test.mjs`, `test/gate.test.mjs` | 70 tests total; `stub-port.mjs`/`dom-stand-in.mjs` imported directly from `faces/ledger/test/`, the same precedent `faces/notice`/`faces/held`/`faces/receipt` already set (req/99 §5) |

## Graph = togglable lens, not a default view (app req/08 §14, backend graph-route ruling)

`req/08 §14-5`'s ruling on the main surface's own graph-traversal *mode* is that atlas's default stays document view, with graph/timeline as switchable lenses over the same node population -- and, separately, `§14-5`'s closing paragraph and `req/03 §6-2 8-K` both state that this standalone `graph` face (F-4, "what has been touched twice or more") answers a *different* question from atlas's own graph-mode preview ("trace a claim's proof chain") and the two coexist rather than merge. Consistent with both rulings, this face is not wired as any shell's default-mounted face: `shell/demo/faces/*` (the only mount points this repo declares today) are untouched by this lane, so `faces/graph` is reached the same way `faces/receipt` is -- mounted directly, by a caller that already knows it wants this question answered, not shown automatically. No `shell/` source line changed in this lane (`grep -rn "faces/graph" shell/` = 0 hits outside this face's own directory).

## Placement and semantics, written before code

`req/100_PLACEMENT_SPEC.md §7` and `req/99_SEMANTICS_REGISTRY.md`'s additive drift block for this face were both written before `graph.mjs` existed, per this lane's own brief.

## Five-principles checklist (`INHERITED_PRINCIPLES.md §3c`)

1. **template-form** -- PASS. Reuses the same `receipt-row.mjs` 8-column grid and the same `row-order.mjs`/`checkable.mjs` decide-parts every other face that needs them already reuses; no second row shape or second ordering implementation invented.
2. **lightweight+bench** -- PASS with a figure: `node tools/bench.mjs` -> **median 7.06ms** for `view(state)` over 1,000 states of 60 transformations each (budget 300ms). Mount ms and real paint remain unmeasured, the same open axis every other face states.
3. **english+comments** -- PASS. All source English; the one literal Japanese excerpt drafted while writing `declaration.mjs`'s `UNDRAWN` entry was caught by this face's own `no-borrowed-symbol` gate and `parts/tools/boundary.mjs`-style ASCII-only test before this README was written, and rephrased in English -- recorded here rather than silently fixed with no trace.
4. **always-CRUD** -- read (R) is the whole of what this face does, by design, and states so structurally (`ACTS = Object.freeze([])`) rather than by omission.
5. **DB-principle** -- PASS. No store: every value is read fresh from the port on each `read()` (`caller.fold(READS.transformations)`), nothing cached or reconstructed client-side; fixtures are declared-rebuildable (`node tools/fixture.mjs`), never hand-edited.

## `[ ]` -- not done, and not called done

- `[ ]` **No independent re-run.** Every number in this README is this lane's own first pass. `req/03 §2`'s `[●]` requires a second, independent session's re-verification, the same discipline every other face in this tree still carries as `[◐]`.
- `[ ]` **RC-5 (no control on this screen reaches another face) is open here too.** There is no way to arrive at this face except by mounting it directly with a port; no row on `faces/ledger`/`faces/held`/`faces/receipt` links here yet.
- `[ ]` **Not registered with the harness.** `tools/faces.json` still lists only fixtures; this lane's write scope was `faces/graph/` (plus the one new glyph in `parts/src/glyph-sheet.mjs`), so the row was not added.
- `[ ]` **No interaction pass (req/38 SS24k)** was performed for this face -- this screen's only interactive controls are its two native `<details>` disclosures (`why`, `legend`), the same population `faces/receipt`'s own interaction pass exercised, but this lane did not repeat that instrument here.
- `[ ]` **Narrow-width real-window capture is absent.** `real_window.ps1` opens at a fixed 1440x900; `tools/shoot.mjs`'s headless narrow (720px) and wide (1280px) captures are the only other-viewport evidence.
- `[ ]` **The `unidentifiable` (no-usable-id) path is exercised only in `test/graph.test.mjs`**, not in any fixture a real renderer photographed.
- `[ ]` **`get_transformations`'s pagination has never reached a real `gx serve`.** `caller.fold()` is the same walk-to-the-end contract `faces/ledger` uses, but this face's own reads, like every other face's, have never been run against a live server.
- `[ ]` **A destructive fault-injection sweep across every shared AC (AC-F1..AC-F4 style) was not repeated here.** The genuine negative control performed (above) targeted this face's own defining property (edge resolution); the four generic negative-truth-ledger ACs are inherited unmodified from the shared parts every other face already fault-injected.

## Seen and not seen (denominator)

- The retired tree's graph face: **0 lines**, by construction. Its `FACE.json` was not opened -- the two-method, eleven-mark contract in `req/03 §3-1` and the one-sentence requirement in `req/03 §5` were read from the requirement documents, not extracted from the reference tree's source.
- `gx-api` handlers: **0 lines**. The members this face looks for (`at`/`actor`/`effect`/`verdict`/`path`/`digest`/`prev`/`sequence` on each transformation) are what this face looked for, not what the server is known to send -- `membrane/wire-fields.json`'s own hole statement confirms this domain contributes no fields yet.
- `glovrex_web/req/phase/app_graph.req.md`: **0 lines** (not located; not opened either way).
- Real `gx serve`: **0 calls** from this face.

## Retrofit round 4 (Owner #340) -- 2026-08-25

Owner #340's reading of this tree was that it is monotone, hard to grasp at a glance and hard to operate. Five things landed here against that, all of them inside this directory; nothing in `parts/`, `shell/` or `tokens/` was touched.

1. **A stat band at the head of the screen.** Four figures, every one a count this face already computed: `touches` (every transformation read), `linked` (touches whose named predecessor this window also read, on the same path -- the one fact this screen computes that no other face has, and until now visible only as a 14px elbow), `reversed`, `undrawn` (declared links whose far end left the window). Zero-inclusive; a count the read never gave draws a dash and never a zero. Four and not five because five equal columns at 720px left a noun about seventy pixels wide and the photograph came back reading `NOT DR...` -- and because the header line one row above already states the path count and the repeated-path count.
2. **Every path group is a box.** The head carries the path, its own touch count, and the verdict recorded on that path's most recent touch, drawn as a filled badge. The `<h3>` that used to state the first two facts is gone: they are the head now, not a heading above rows. The two states with no group to draw (answered-with-nothing, and a read that did not answer) keep a box too -- `0 paths` for the first, a dash for the second, because an empty group and an unread one are different facts.
3. **Design identity.** Every standing on this screen goes through `badge`, so it takes the bed its standing owns rather than a stroke in the body ink. On the shipped fixture three boxes carry one hue and one carries another; on the browser-mount smoke a third appears (Escalate). Read at arm's length from the shot, the boxes separate.
4. **Scrollbar remnants: none, verified.** `grep -n "overflow\|max-height\|height *:" faces/graph/*.mjs` returns 6 hits, all six `overflow-wrap`; `max-height` and a fixed `height:` are 0. A tree test now holds it: the only node in the drawn tree declaring `max-height`/`overflow-x`/`overflow-y` is the shared `detail-pane`.
5. **A runtime footer.** `render` is measured with `performance.now()` around the work `view()` already does, on the call that drew the screen -- not estimated and not a build constant. `read` names the source only when the read answered, and draws a dash when it did not.

Also closed here: `node tools/gate.mjs` used to load the module, evaluate nothing and exit 0 -- an exit code that read as "every check held" while no check had been applied. It now runs the checks over the three fixture states and prints them.

**Measured after, not claimed**: `node --test "test/*.test.mjs"` = **90 pass / 0 fail**; `node tools/gate.mjs` = **16/16 held, exit 0**; `node tools/bench.mjs` = **median 5.696ms** against a 300ms budget (the round before this one measured 8.833ms on the same instrument; both are noise around the same figure and neither is a second population point); `node tools/shoot.mjs` = 7 captures, **overlaps 0 / repeated rows 0 / oversize glyphs 0 / clipped-without-full 0 / under tap budget 0 / horizontal overflow 0** on every one; `node tools/browser-mount-smoke.mjs` = ready, 334 elements, 2 path groups, 2 chained rows, 1 outside annotation.

### `[ ]` -- what this round did not close

- `[●]` **CLOSED in round 5 -- see "Retrofit round 5" below.** ~~Every row still repeats its own path, which its box head now also states.~~ Four copies of one string inside one box is the densest redundancy on this screen. The fix is one argument (`columns:` without the path column, for rows drawn inside a path box) and it was left alone deliberately: it is a change to the shared row grammar rather than one of this round's atoms, and `req/98` §1-1 already specifies the cell budget that replaces this row form entirely.
- `[ ]` **The screen still states its failures louder than its successes.** The one edge that could not be drawn gets two lines of prose inside the box; the three that were drawn get an elbow each and a figure in the band. That is better than before and it is not yet right; `req/98` §3-2 turns the annotation into a counted class with a stub mark.
- `[ ]` **`code: UNAUTHORIZED` is still drawn as a bare screaming enum** on the unread state. It is the engine's own word and must not be rephrased, but it is drawn with no label saying whose word it is.
- `[●]` **CLOSED in round 5 -- see "Retrofit round 5" below.** ~~No interaction pass, and none is reachable.~~ `mount()` paints the waiting view, paints the view the read answered with, and stops; it installs no listener anywhere. Every row is a real `<button>` with `aria-pressed` and a field count, and pressing one does nothing -- `view()` reads `state.selected` but no code path in this face ever produces a state carrying it, so in a live window the pane is permanently in its no-selection state. `req/98` §3-3 item 2 owns the activation mechanism; it is not built here.
- `[ ]` **Nothing in this round was independently re-run.** Every number above is this lane's own first pass.

## Retrofit round 5 (Owner #348 / #349) -- 2026-08-25

**The row answers now.** Round 4's own list above ended by naming the worst thing on this
face: every row was a real `<button>` carrying `aria-pressed` and a field count, `view()`
read `state.selected`, and no code path anywhere in the face ever produced a state
carrying one -- so in a live window the pane was permanently empty and pressing a row did
nothing. `mount()` installs a press handler, and the same paint that chooses a touch
fills the pane. Which folds are open moved into state at the same time and for the same
reason: the moment a repaint is reachable, an element keeping its own open/shut answer
and this window keeping another is two answers to one question (`req/103` finding 2).

**A menu on the second mouse button**, drawn from the declaration rather than from a
second list. `ACTS` is empty on this face and the menu says so out loud rather than
opening onto nothing; what it offers is `copy value` for the cell the pointer was over
and `copy identity`, with an unavailable item drawn disabled and carrying its reason.
Three properties are structural rather than careful: it cannot survive a repaint (it is
drawn from `state.menu` like everything else), two cannot stack (`state.menu` names one
touch), and it is **in flow** -- `req/03` N-1 was an out-of-flow element drawn on top of a
row, and `tools/gate.mjs` still refuses `position` in this face's source. The cost of
that choice, measured in the renderer rather than read off a picture: the menu is 51px
tall and the rows below it move down by exactly 51px while it is open.
A copy states whether it worked (`data-copied` / `data-copy-failed`), because a control
that looks the same either way is the thing the shell's own copy control already refuses.

**Words removed.** Drawn text on the happy path: **9,479 -> 6,079 characters (-35.9%)**;
visible text in a real renderer at 720px: **860 -> 595 (-30.8%)**. Four cuts, in order of
size: the undrawn census was drawn twice (all seven entries with their reasons, once in
`legend` and once in `omitted`); three legend prose lines restated `structure/child`,
`structure/outside` and the undo chip, which the counted mark table above them already
prints with the declaration's own words; every row drew the path its box head states; and
the pane opened with the group's path and then drew `path in full` underneath with the
identical string. Two smaller ones: the outside-edge annotation said its one fact in two
clauses (178 -> 40 characters, the longer form kept on the node), and each folded control
carried a synonym of its own name (`omitted -- what is not drawn`), replaced by the count
of what is behind it.

**What took the width the echoed column left.** With `path` simply deleted the drawn
tracks ended around 310px of a 780px row and the row read as two clusters with a canyon
between them. `fingerprint` fills it: the column `SCAN_COLUMNS` had to drop when eight
tracks starved `path` to zero pixels, a value that differs on every row where `path` was
identical on all of them, and the one member this face read on every touch and drew
nowhere. It is a declared cut, so the whole digest goes into the pane in the same change.
The fixture's digests were `d001` -- four characters, which `parts/src/serial.mjs`
correctly refuses to cut six characters out of -- and are now 16 hexadecimal characters,
deterministic per sequence.

**Mechanical, not incidental.** One weight scale (`WEIGHT`: 700 figure / 500 label / 400
body) and one wrap route (`WRAP`: `break-word` for prose with `text-wrap: pretty`,
`anywhere` only for a string with no spaces in it) -- and `tools/gate.mjs` now refuses a
bare `font-weight` number, a bare `transition`/`ms` duration, a `border-radius` literal
and a numeric mark size anywhere in this face's source. Each of the four was fired red on
a planted line and green on the legitimate form before being trusted. A fifth check reads
the drawn tree: every mark at or above `MIN_READABLE`, which is the half a source rule
cannot see. Marks: the two at 14 are gone; every size is `P.minReadable` by name.

**Measured after, not claimed**: `node --test` = **113 pass / 0 fail** (67 + 31 + 15);
`node tools/gate.mjs` = **21/21 held over 4 drawn states**, exit 0;
`node --test parts/test/glyph-sheet.test.mjs` = **17/17**, the floor gate green;
`node tools/bench.mjs` = **medians 9.1 / 9.8 / 11.5 / 11.9 / 15.7 / 24.6ms over six runs**
against a 300ms budget. Five other face lanes and a headless renderer were on this
machine throughout, and the spread inside a single run is 5-59ms, so the honest reading
is that this instrument cannot separate this round from round 4's 5.7ms -- only that both
are an order of magnitude inside the budget;
`node tools/shoot.mjs` = **10 captures**, overlaps 0 / repeated rows 0 / oversize glyphs 0
/ clipped-without-full 0 / under tap budget 0 / horizontal overflow 0 on every one;
the open menu measured in the renderer: **51px tall, 678px wide, and the three rows below
it move down by exactly 51px** while the two above it do not move;
`node tools/browser-mount-smoke.mjs` = ready, 295 elements, 2 path groups, 2 chained rows,
1 outside annotation.

### `[ ]` -- what round 5 did not close

- `[ ]` **`unknown` is printed once per row and never varies.** Five copies of a word that
  is the same on every touch this fixture holds, next to a band that already states
  `reversed 0`. It is not removed because `req/768` AC-7 requires one chip per touch and
  three tests in this face assert exactly that; changing it is a change to that
  requirement, not to this face.
- `[ ]` **`N fields` is printed once per row and, on this population, never varies.** It is
  drawn by `parts/src/receipt-row.mjs` `selectableRow()`, which this lane may not edit.
- `[ ]` **The menu draws no mark.** The canonical sheet has no mark meaning "copy", a face
  may not add one, and drawing an existing mark for a meaning it does not carry is worse
  than drawing none. Named for the seat.
- `[ ]` **The menu's own buttons carry inline colour, border and cursor.** Nothing in
  `parts/src/surface.mjs` names `[data-part="face-menu"]`, so there is no rule for them to
  come from -- the same shape the act gutter had before `37f7ff3`. Named for the seat.
- `[ ]` **`code: UNAUTHORIZED` is still drawn as a bare screaming enum** on the unread
  state, with no label saying whose word it is. Unchanged from round 4.
- `[ ]` **Nothing in this round was independently re-run.** Every number above is this
  lane's own first pass.
