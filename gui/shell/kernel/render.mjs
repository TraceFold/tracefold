// SPDX-License-Identifier: Apache-2.0
// Drawing. The frame is built once and then only the parts that differ are touched.
//
// The system this was derived from emptied the window and rebuilt the whole tree on every
// change, including a change of focus, which meant every face was torn down and started
// again whenever anything at all happened. It also had no way to stop a face, so nothing
// that had been started was ever ended. Here the two go together: a face is stopped
// exactly when the place it stands in stops holding it, and the counters say so.
//
// Nothing in this file is written as markup text. Every string that came from outside --
// a face's title, a space's name -- is set as textContent, so a face called
// `</div><script>` is a face with a strange name and not a hole.

import { isLeaf } from './tree.mjs';
import { DOCK_SIDES, shortDigest } from './layout.mjs';
// req/867: `railFaces` leaves this import with `#drawLauncher`, which was its only caller
// here. It is still the population behind W14 and is still used by tools/ac_population.mjs.
import { DOCK_RULES } from './manifest.mjs';
import { mark } from './marks.mjs';
import { changing } from './mount.mjs';
import { commandFor } from './command.mjs';
import { corpusOf, search, FACETS, PALETTE_SAID } from './palette.mjs';
import { menuFor } from './menu.mjs';
import { NOT_WIRED } from './measures.mjs';

const el = (tag, className, text) => {
  const made = document.createElement(tag);
  if (className) made.className = className;
  if (text !== undefined) made.textContent = text;
  return made;
};

const button = (className, label, said) => {
  const made = el('button', className);
  made.type = 'button';
  made.setAttribute('aria-label', label);
  made.title = said ?? label;
  return made;
};

/**
 * Owner #348 (3). A tab's mark was 12px, which puts the 2-unit stroke of a 24-unit
 * design under one pixel -- it does not read as small, it reads as broken, and it is
 * what the Owner is seeing when a mark looks cut off. It takes the readable floor now,
 * the same one the rail already happened to be at.
 *
 * The sash keeps 10 and is the one deliberate exception in this application: it is not
 * an icon and carries no meaning a reader decodes -- it is the grip on a divider, and
 * the thing it must do is be small enough not to look like content. It is named in
 * parts/test/glyph-sheet.test.mjs's exception list with that reason rather than being
 * quietly under the floor.
 */
export const MARK_SIZE = Object.freeze({ rail: 16, tab: 16, sash: 10 });

/**
 * What a count slot says when there is no count to say.
 *
 * A dash and a zero are different facts and this frame must not confuse them (req/784
 * L-04). Zero means a face is standing and has measured its population as empty. The
 * dash means this face is not standing anywhere in this space right now, so nothing has
 * been read for it -- which is a state the sidebar is showing on purpose, not a gap.
 * Writing `0` there would be the frame reporting a measurement nobody took.
 */
export const COUNT_UNREAD = '--';

/**
 * Where a face's own population count is read from.
 *
 * The frame may not know a face id (W1) and may not hold a list of them (W14), so it
 * cannot ask a particular face a particular question. What it can do is read what any
 * face has already drawn: every face draws a band at its head whose first segment is
 * its own population, stated as a number and a noun (`parts/src/surface.mjs`
 * statBand()). This reads that segment out of whatever is standing at a host, keyed by
 * the `data-face` attribute a mounted face puts on its own root -- which is the same
 * attribute `shell/app/boot.mjs` already uses to tell a face that drew from one that
 * did not.
 *
 * That makes the number live in the strongest sense available here: it is not a figure
 * the frame computed, or a figure a face reported through a side channel, but the
 * figure the face is at this moment displaying. The two cannot disagree.
 */
export const COUNT_SOURCE = Object.freeze({
  face: '[data-face]',
  band: '[data-part="stat-band"] [data-role="segment"]',
  value: 'data-value',
  noun: 'data-noun',
  unread: 'unread',
  defaultNoun: 'records',
});

/**
 * The deciding half of the census, with no document in it.
 *
 * The reach into the DOM is four `querySelector` calls and is not interesting; what is
 * interesting is what happens to a reading that is missing, empty or explicitly unread,
 * and that is the part a test can hold. Separated for exactly that reason -- the same
 * division `changing()` was pulled out of this file for under W8.
 *
 * The rule, in one line: only a number that a face is actually displaying becomes a
 * count. Everything else is left out of the map, and a slot with nothing in the map
 * draws the dash. There is no branch anywhere here that produces a zero.
 *
 * @param {{id: string|null, value: string|null, noun: string|null}[]} readings
 */
export function censusOf(readings) {
  const found = new Map();
  for (const reading of readings ?? []) {
    const id = reading?.id;
    const value = reading?.value;
    if (!id) continue;
    if (value === null || value === undefined || value === '' || value === COUNT_SOURCE.unread) continue;
    found.set(id, { value: String(value), noun: reading.noun || COUNT_SOURCE.defaultNoun });
  }
  return found;
}

/** What a slot says, given whatever the census had for it. */
export function countText(found) {
  return found ? found.value : COUNT_UNREAD;
}

/**
 * What a count slot says, in each of the three states it can actually be in.
 *
 * req/811 §8-2b is the reason there are three. There were two sentences for three states,
 * and the missing one was load-bearing: `--` is produced when the census found no value
 * (`countText` above), and it was explained as "this face is not standing anywhere in this
 * space". Those are different facts. A face that stands on the stage and has not answered
 * yet also has no census value, so on first paint the window asserted `on` -- meaning
 * *stands somewhere* -- on five of six items while the tooltip on the very same button
 * said the face stood nowhere, and broadcast the first of the two to assistive technology
 * as `aria-pressed="true"`. The window stated a proposition and its negation about one
 * face at one instant, six times, on a product whose whole claim is that it never states
 * what it has not verified.
 *
 * So the dash is split by what is true: a face that is placed and has not reported is
 * `standing`, a face that is placed nowhere is `unplaced`, and a number is a number.
 */
export const COUNT_SAID = Object.freeze({
  unplaced: 'this face is not standing anywhere in this space, so nothing has been read for it',
  standing: 'this face is standing here and has not reported a count yet, so there is nothing to write',
  read: (found) => `${found.value} ${found.noun}, as this face is drawing it now`,
});

/** The three, named, so a slot's state is a value and not a pair of booleans read twice. */
export const COUNT_STATE = Object.freeze({ READ: 'read', STANDING: 'standing', UNPLACED: 'unplaced' });

/** A region, in the words a reader uses, for the standing column's rows (req/811 B-1). */
export const REGION_SAID = Object.freeze({
  stage: 'the stage',
  left: 'the left dock',
  right: 'the right dock',
  bottom: 'the bottom dock',
  nowhere: 'nowhere in this space',
});

/** The one group the window bar carries (req/867; was two, above and below the sidebar's
 *  crease, at req/811 §4-1). `STANDING` is gone with the column it headed -- the tab strip
 *  is the face selector and the palette is where a nowhere-standing face gets placed. */
export const SIDE_GROUP = Object.freeze({ SPACE: 'SPACE' });

export class Frame {
  #root;

  #read;

  #mounted;

  #port;

  #notices;

  #act;

  #history;

  #view;

  #parts = {};

  #standing = { said: '', outcome: '' };

  #lastState = null;

  #countWatch = null;

  #pinWatch = null;

  constructor({ root, read, mounted, port, notices, act, history, viewpoint }) {
    this.#root = root;
    this.#read = read;
    this.#mounted = mounted;
    this.#port = port;
    this.#notices = notices;
    this.#act = act;
    // Optional: a Frame built without history draws the depth without controls rather
    // than drawing controls that cannot do anything.
    this.#history = history ?? null;
    this.#view = viewpoint;
    this.#build();
  }

  #build() {
    const shell = el('div', 'shell');
    // req/867 (Owner #416追記3). There is no left sidebar. What follows is the whole of
    // why, because the element being gone is the cheap half and the derivation is the
    // half that has to survive the next person asking for it back.
    //
    // The question was not "should the sidebar go" but "what is the smallest set of
    // roles a persistent left region can hold such that each one is needed, is not
    // already served by the tabs or by a window bar, and is sayable in one sentence".
    // Every candidate was run at that gate and every one of them fell:
    //
    //   face selection      -- the tab strip is the face selector, measured (req/811
    //                          §2-2, §2-3: pressing a launcher row moved no breadcrumb,
    //                          no tab and no rail state, because it was never
    //                          navigating). Served. OUT.
    //   the SPACE mode      -- `verify` / `inspect` is a property of the whole window,
    //                          not of its left edge. A global mode belongs on a bar that
    //                          spans the window it modifies. Moved to `.topbar`. OUT.
    //   find                -- req/811 §8-5 already ruled it: no permanent search bar,
    //                          one invoked palette, because a bar spends chrome on every
    //                          frame for a capability used on almost none of them. The
    //                          control that opens it is window chrome. Moved. OUT.
    //   theme               -- window chrome, and req/811 B-6 had already moved it out
    //                          of the nav landmark. Moved. OUT.
    //   detail storage      -- SS690's sanctioned one-pane sidebar, which req/811 §4
    //                          itself ruled "KILL as the shell's role, ADOPT as
    //                          face-level furniture". It belongs inside held / ledger /
    //                          receipt, never in shell chrome. OUT.
    //   a persistent left
    //   region holding a
    //   face                -- `aside.dock-left` already IS one, holds real faces, and is
    //                          sized by a real sash. A second persistent left region is
    //                          the duplication, not the cure. OUT.
    //
    // The set is empty, so the region is gone and its 196px go back to the faces.
    //
    // One role did survive the sweep, and it is the reason this is a move rather than a
    // deletion: the standing column was the ONLY press in the window that could place a
    // face standing nowhere (`#summon` had exactly one caller). Owner #372 makes
    // functionality a floor, so that press is not allowed to vanish with the column that
    // happened to host it. It moves to where §8-5's own reasoning puts it -- into the
    // palette, invoked rather than resident, where `corpusOf` was already listing every
    // nowhere-standing face and already saying "place it first" on a row it then refused
    // to let anybody press. See `#drawPalette`.
    //
    // What this supersedes in req/811: §4 candidate B and its B-1..B-9, §4-2, §6-1's
    // R-1..R-7, and the §7 gate 11 / §9-4 col F reading that "removing a feature is never
    // a valid answer to a redundancy finding" forbids this. It does not: no feature is
    // removed here. Every role either moved to a surface that fits it better or was
    // already served twice. §7 gate 11 governs answers to redundancy findings; this is an
    // answer to a role derivation, and it keeps the floor gate 11 exists to protect.
    //
    // SS551's own rule survives untouched: the rail carries space nouns only and a face
    // never stands in it. It is horizontal now instead of vertical, which SS551 never
    // spoke to.
    const rail = el('nav', 'rail');
    rail.setAttribute('aria-label', 'spaces');
    const middle = el('div', 'middle');
    const across = el('div', 'across');
    const docks = {};
    for (const side of DOCK_SIDES) {
      docks[side] = el('aside', `dock dock-${side}`);
      docks[side].dataset.side = side;
      docks[side].setAttribute('aria-label', `${side} dock`);
    }
    const stage = el('main', 'stage');
    stage.setAttribute('aria-label', 'stage');
    across.append(docks.left, this.#sash('left'), stage, this.#sash('right'), docks.right);
    middle.append(across, this.#sash('bottom'), docks.bottom);
    const strip = el('footer', 'strip');
    // The one group label that is left, and it is not decoration. `verify` and `inspect`
    // are verbs. Two bare verbs sitting in a window bar read as things to PRESS TO DO,
    // and they are neither -- they are the two modes this window can be in. The label is
    // the word that makes a mode look like a mode, so it stays where the crease used to
    // be doing that job.
    const spaceLabel = el('div', 'rail-label-head', SIDE_GROUP.SPACE);
    // req/811 B-6: chrome acts do not live inside a navigation region. `theme` sits
    // beside the nav on the bar, never inside it.
    const chrome = el('div', 'chrome-acts');
    chrome.setAttribute('aria-label', 'window controls');
    // req/867: the window bar the sidebar's removal pays for.
    //
    // req/822_c7 declined this exact bar with a number, and the number was right at the
    // time: a top bar costs a whole `--bar-h` of vertical room across the window, and
    // back then the sidebar was a column that already existed, so putting `find` at its
    // head cost nothing extra. That trade is now reversed, and it is worth doing the
    // arithmetic in the open rather than asserting it. Measured on the real window at
    // 1440x900 (req/867 §2, shell/record/req867_sidebar/*_facts.json), not estimated:
    // the sidebar was 196px x 900px = 176,400px^2 of permanent chrome; this bar is
    // 1440px x 37px = 53,280px^2. The faces are up 123,120px^2, about 70% of what the
    // column was taking, and they gain it on the axis that was actually starved -- the
    // stage went 920x595 -> 1116x562, which is +196px of WIDTH against -33px of height,
    // and a table of fixed-width columns is hurt by a narrow pane far more than a short
    // one. (37px, not the bare 34px of --bar-h: the border and padding are real pixels.)
    const topbar = el('header', 'topbar');
    topbar.setAttribute('aria-label', 'window bar');
    const finder = el('div', 'bar-find');
    finder.append(this.#findAct());
    // req/884: the dock presses live on the bar, beside the other window chrome, because
    // a dock is a property of the window and not of any face inside it -- the same
    // argument req/867 used to move SPACE here. Its own holder so `#drawDockActs` can
    // replace exactly these each paint without touching find or theme.
    const dockActs = el('div', 'chrome-docks');
    dockActs.setAttribute('aria-label', 'window regions');
    topbar.append(spaceLabel, rail, finder, dockActs, chrome);
    shell.append(topbar, middle, strip);
    this.#parts = {
      shell, topbar, rail, chrome, dockActs, middle, across, docks, stage, strip,
    };
    this.#root.append(shell);
    chrome.append(this.#themeToggle());
    this.#buildStrip(strip);
    this.#buildPalette(shell);
    this.#buildMenu(shell);
    this.#watchCounts(shell);
    this.#watchPins(shell);
    // The strip and the sashes are built once, so their menus are bound once, here.
    strip.addEventListener('contextmenu', (event) => this.#openMenu(event, 'strip', {
      digest: this.#parts.stripDigest?.title ?? null,
      suite: this.#parts.stripSuite?.textContent ?? null,
    }));
    for (const side of DOCK_SIDES) {
      const sash = this.#parts.shell.querySelector(`.sash-${side}`);
      sash?.addEventListener('contextmenu', (event) => this.#openMenu(event, 'sash', {
        index: this.#spaceIndex,
        side,
        size: this.#lastState?.spaces[this.#spaceIndex]?.docks[side]?.size ?? 0,
      }));
    }
  }

  /**
   * The palette's own control, so the capability is not keyboard-only.
   *
   * A chord is the fast way in and must not be the only way in: a person who never learns
   * the chord would otherwise have a feature that exists and is unreachable, which scores
   * as a feature on every inventory and as nothing at all on a screen.
   */
  #findAct() {
    const said = 'find a face by where it stands, and take its address';
    const made = button('chrome-act', 'find', said);
    made.append(mark('space', MARK_SIZE.rail));
    made.append(el('span', 'chrome-act-label', 'find'));
    made.addEventListener('click', () => this.openPalette());
    return made;
  }

  // ---- the chrome menu: right-click on the frame itself (req/810 GAP-3) --------------

  /**
   * Owner #366 asked for right-click across the whole surface. r4 gave one to all six
   * faces and the chrome had none: sidebar rows, tabs, dock heads, strip and sash were
   * bare, so "全域" meant six faces and nothing else.
   *
   * Every row is drawn, including the ones that cannot act -- those carry their reason
   * and are honestly disabled, never hidden and never silently inert. That is the same
   * `why` the outcome itself now requires (`state.mjs` #record), rendered.
   */
  #openMenu(event, target, context) {
    event.preventDefault();
    const menu = this.#parts.menu;
    menu.replaceChildren();
    for (const item of menuFor(target, context)) {
      const made = button('chrome-menu-row', item.label, item.why ?? item.said);
      made.append(el('span', 'chrome-menu-label', item.label));
      made.append(el('span', 'chrome-menu-said', item.why ?? item.said));
      if (item.why) {
        made.disabled = true;
        made.dataset.why = item.why;
      } else if (item.act) {
        made.addEventListener('click', () => { this.closeMenu(); this.#act(item.act.verb, item.act.args); });
      } else if (item.copy) {
        made.addEventListener('click', () => { this.closeMenu(); this.#copyText(item.copy); });
      }
      menu.append(made);
    }
    menu.hidden = false;
    // Placed inside the window, never off the edge of it: a menu a person has to scroll
    // the page to read is a menu that was not opened where they clicked.
    const box = this.#parts.shell.getBoundingClientRect();
    const width = 280;
    const left = Math.min(event.clientX, box.width - width - 8);
    const top = Math.min(event.clientY, box.height - menu.offsetHeight - 8);
    menu.style.left = `${Math.max(8, left)}px`;
    menu.style.top = `${Math.max(8, top)}px`;
    menu.dataset.target = target;
  }

  closeMenu() {
    const menu = this.#parts.menu;
    if (menu) menu.hidden = true;
  }

  get menuOpen() { return this.#parts.menu ? this.#parts.menu.hidden === false : false; }

  #buildMenu(shell) {
    const menu = el('div', 'chrome-menu');
    menu.hidden = true;
    menu.setAttribute('role', 'menu');
    menu.setAttribute('aria-label', 'what this part of the window offers');
    shell.append(menu);
    this.#parts.menu = menu;
    // One dismissal path for the menu, shared with everything else that closes: a
    // pointerdown anywhere that is not the menu, or Escape.
    shell.ownerDocument.addEventListener('pointerdown', (event) => {
      if (!this.menuOpen) return;
      if (event.target.closest('.chrome-menu')) return;
      this.closeMenu();
    });
    shell.ownerDocument.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') this.closeMenu();
    });
  }

  /** [B4] Copy from the chrome menu.
   *
   * This closed the menu and then returned silently: no clipboard was an early return,
   * a rejected write was a swallowed `catch`, and a successful write drew nothing. All
   * three outcomes looked identical from the reader's chair -- the menu shut, and
   * whether the text was taken was something you found out by pasting. The window now
   * says which of the three happened, in the strip where every other outcome is
   * reported, and announces it. */
  #copyText(text) {
    const clip = typeof navigator !== 'undefined' ? navigator.clipboard : undefined;
    if (!clip || typeof clip.writeText !== 'function') {
      this.#saidCopy('this window could not reach a clipboard, so nothing was taken', 'refused');
      return;
    }
    clip.writeText(text).then(
      () => this.#saidCopy('taken, and on the clipboard', 'moved'),
      () => this.#saidCopy('the clipboard refused, so nothing was taken', 'refused'),
    );
  }

  /** A copy is not an act on the line -- it changes nothing that is true and belongs in
   * no history -- so it is reported without being recorded. */
  #saidCopy(words, outcome) {
    this.#standing = { said: words, outcome };
    if (this.#parts.stripSaid) {
      this.#parts.stripSaid.textContent = words;
      this.#parts.stripSaid.dataset.outcome = outcome;
      this.#parts.stripSaid.title = words;
    }
    this.announce(words, outcome);
  }

  // ---- the palette: invoked, never resident (req/811 §8-5) ---------------------------

  #buildPalette(shell) {
    const palette = el('div', 'palette');
    palette.hidden = true;
    palette.setAttribute('role', 'dialog');
    palette.setAttribute('aria-modal', 'true');
    palette.setAttribute('aria-label', 'find a view');
    const field = el('input', 'palette-field');
    field.type = 'text';
    field.setAttribute('aria-label', 'what to find');
    field.placeholder = `${Object.keys(FACETS).map((f) => `${f}:`).join('  ')}  or any word`;
    const legend = el('div', 'palette-legend');
    // The grammar is drawn, not hidden in a tooltip: req/811 §7 gate 7 makes semantics
    // reachable only through `title=` a failure, and an axis nobody can see is an axis
    // nobody uses.
    for (const [name, means] of Object.entries(FACETS)) {
      const row = el('span', 'palette-facet');
      row.append(el('code', 'palette-facet-name', `${name}:`), el('span', 'palette-facet-said', means));
      legend.append(row);
    }
    const said = el('p', 'palette-said', PALETTE_SAID.empty);
    const list = el('ul', 'palette-results');
    palette.append(field, legend, said, list);
    shell.append(palette);
    this.#parts.palette = palette;
    this.#parts.paletteField = field;
    this.#parts.paletteSaid = said;
    this.#parts.paletteResults = list;

    field.addEventListener('input', () => this.#drawPalette());
    field.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') { this.closePalette(); return; }
      if (event.key !== 'Enter') return;
      const first = list.querySelector('.palette-row');
      if (first) first.click();
    });
    palette.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') this.closePalette();
    });
  }

  /** Open it, and hand the person the caret. Nothing about the window's state changes. */
  openPalette() {
    const palette = this.#parts.palette;
    if (!palette) return;
    palette.hidden = false;
    this.#parts.shell.dataset.palette = 'open';
    this.#parts.paletteField.value = '';
    this.#drawPalette();
    this.#parts.paletteField.focus();
  }

  closePalette() {
    const palette = this.#parts.palette;
    if (!palette) return;
    palette.hidden = true;
    this.#parts.shell.removeAttribute('data-palette');
  }

  get paletteOpen() { return this.#parts.palette ? this.#parts.palette.hidden === false : false; }

  #drawPalette() {
    const list = this.#parts.paletteResults;
    const query = this.#parts.paletteField.value;
    const found = search(query, corpusOf(this.#read, this.#lastState));
    this.#parts.paletteSaid.textContent = found.said ?? `${found.rows.length} here`;
    list.replaceChildren();
    for (const row of found.rows) {
      const item = el('li', 'palette-row-holder');
      const made = button('palette-row', row.title, row.address ?? row.why);
      made.append(el('span', 'palette-row-name', row.title));
      made.append(el('span', 'palette-row-box', row.box));
      // The address, drawn on the result. This is the whole point of the ruling: a hit is
      // a reproducible command, not a place a scrollbar happens to be.
      made.append(el('code', 'palette-row-address', row.address ?? row.why));
      if (row.land) {
        made.addEventListener('click', () => {
          this.closePalette();
          this.#act(row.land.verb, row.land.args);
        });
      } else {
        // req/867: this row used to be honestly disabled, and the honesty was real but
        // the disablement was a capability going missing. The row says `this face stands
        // nowhere, so there is no view to reproduce; place it first` -- and until the
        // standing column was derived away, the only press in the entire window that
        // could place it lived in that column. So the palette told a reader what to do
        // and refused to let them do it, two clicks from a list that could.
        //
        // Now the row IS the place-it press. `#summon` is unchanged and this is its
        // second caller: a stage face opens as a tab where the person is looking, a dock
        // face goes to its declared side. The row redraws itself on the next paint
        // carrying a real address, because it now has one.
        const face = this.#read.byId?.get(row.id);
        if (face && this.#lastState) {
          made.dataset.why = row.why;
          made.dataset.places = 'true';
          const said = `${row.title} stands nowhere; press to place it`;
          made.title = said;
          made.setAttribute('aria-label', said);
          made.addEventListener('click', () => {
            this.closePalette();
            this.#summon(this.#lastState, face);
          });
        } else {
          // Honestly disabled, with the reason on it -- the same convention every other
          // unpressable control in this frame already uses. Reached only when the face is
          // not in the manifest read, which is a row about nothing pressable.
          made.disabled = true;
          made.dataset.why = row.why;
        }
      }
      item.append(made);
      list.append(item);
    }
  }

  /**
   * The theme act, out of the nav landmark and otherwise unchanged (req/811 B-6).
   *
   * It reads the theme off the document rather than off a state closure, because this
   * control is built once and the state it would have closed over is the state at build
   * time -- the same stale-closure shape `req/103` Finding 1 records on a face.
   */
  #themeToggle() {
    const made = button('chrome-act', 'theme', 'switch this window between its two inks');
    made.append(mark('theme', MARK_SIZE.rail));
    made.append(el('span', 'chrome-act-label', 'theme'));
    made.addEventListener('click', () => {
      const now = document.documentElement.getAttribute('data-theme');
      this.#act('theme:set', { theme: now === 'light' ? 'dark' : 'light' });
    });
    return made;
  }

  /**
   * The dock presses (req/884, Owner: 標準で画面分割はなし).
   *
   * These exist because the DEFAULT changed, and the default could not change safely
   * without them. `openingState` now opens every dock shut, so a first-time reader gets
   * one pane. That is only a default change -- and not a capability removal -- if what
   * was closed can be opened again, and before this the window failed that test twice:
   *
   *  - No button anywhere called `dock:open`. Not one. A dock became visible only as a
   *    side effect of `dock:add` changing `faces.length`, so with the docks shut on
   *    arrival a pointer-only reader had no way back to Held or Notice at all.
   *  - The keyboard could reach two sides of three. `keyArguments` picked the side with
   *    `hit.chord.endsWith('j') ? 'bottom' : 'left'`, and an else-branch is how `right`
   *    came to be unreachable without anybody deciding it should be.
   *
   * So the presses are the pointer half and `keys.mjs` gains the missing chord for the
   * keyboard half. One press per side that actually holds a face -- a toggle for a dock
   * with nothing in it would be a control that does nothing, which is the defect req/811
   * §8-2b is about. Each says which it is, carries `aria-pressed`, and goes through
   * `#act` like everything else, so opening a dock is on the undo line and in the receipt.
   */
  /**
   * Redrawn from the state each paint, never built once and left. A toggle built at
   * `#build` time would close over the dock's opening state and then lie about it for
   * the rest of the session -- the stale-closure shape `req/103` Finding 1 records, and
   * the reason `#themeToggle` reads the document rather than a closure.
   */
  #drawDockActs(state) {
    const holder = this.#parts.dockActs;
    if (!holder) return;
    holder.replaceChildren(...this.#dockToggles(state));
  }

  #dockToggles(state) {
    const space = state.spaces[state.space];
    const made = [];
    for (const side of ['left', 'right', 'bottom']) {
      const dock = space.docks[side];
      if (!dock || dock.faces.length === 0) continue;
      const open = dock.open === true;
      const press = button(
        'chrome-act',
        side,
        `${open ? 'shut' : 'open'} the ${side} dock (${dock.faces.join(', ')})`,
      );
      press.setAttribute('aria-pressed', String(open));
      // No mark. The first draft appended `mark('dock', ...)`, and the demo checks caught
      // it: "3 marks were asked for by a name nobody drew" -- one per press. `dock` is
      // called elsewhere in this file but the sheet does not define it, so I had verified
      // the wrong thing (which names this file CALLS, not which the sheet DRAWS) and the
      // check knew better. A word is also the honest control here: these three read
      // `left` / `right` / `bottom`, which is the whole of what they do, and adding a
      // glyph would have cost the bar a second row -- which it did, until this came out.
      press.append(el('span', 'chrome-act-label', side));
      press.addEventListener('click', () => {
        this.#act('dock:open', { index: state.space, side, open: !open });
      });
      made.push(press);
    }
    return made;
  }

  /**
   * `narrow` is gone, and this note is here because a control disappearing is exactly the
   * kind of thing that should not be allowed to happen quietly (req/867, Owner #372).
   *
   * It was not a window capability that got cut. `narrow` had one object in the world --
   * the sidebar's own width, `--side-w` between 196px and 46px -- and it did nothing else
   * to anything else. It never went through `#act`, never touched the line, never changed
   * what stood where or what any count said. With the sidebar derived away, the control
   * has no referent: there is nothing left for it to narrow. A control is retired with its
   * object, which is a different act from answering a redundancy finding by deleting a
   * feature, and it is the one §7 gate 11 does not forbid.
   *
   * The underlying want -- "give this region less room" -- is still served, and better:
   * `aside.dock-left` is a real persistent left region holding a real face, and it is
   * sized by a real sash that reports its pixels, rather than by a two-position toggle.
   *
   * Flagged in req/867 as an open question rather than settled by this lane: if the Owner
   * wants a `narrow` on the window bar, it needs a new object first, and this note is
   * where the next person should start.
   */

  #sash(side) {
    const made = el('div', `sash sash-${side}`);
    made.dataset.side = side;
    made.setAttribute('role', 'separator');
    made.setAttribute('aria-label', `${side} dock size`);
    made.addEventListener('pointerdown', (event) => this.#dragDock(event, side));
    return made;
  }

  #buildStrip(strip) {
    const digest = el('span', 'strip-digest');
    const said = el('span', 'strip-said');
    // [B6] Was one span reading "N behind, N ahead, N dropped" -- the best information
    // on the surface, rendered as text no pointer could use, while the only route to
    // undo was `mod+z`. The counts are now the labels ON the controls, so the number
    // and the act that consumes it are the same object rather than a caption beside a
    // capability the window never offered. `dropped` stays text: it names history this
    // window has thrown away, and there is no act that recovers it.
    const counts = el('span', 'strip-counts');
    const undo = button('strip-step', 'undo', 'undo the last shell act');
    const redo = button('strip-step', 'redo', 'redo the last undone act');
    undo.addEventListener('click', () => this.#step('undo'));
    redo.addEventListener('click', () => this.#step('redo'));
    const dropped = el('span', 'strip-dropped');
    counts.append(undo, redo, dropped);
    this.#parts.stripUndo = undo;
    this.#parts.stripRedo = redo;
    this.#parts.stripDropped = dropped;
    const checks = el('span', 'strip-checks');
    // SS551: the persistent bottom status bar carries LIVE measured numbers --
    // suite counts (.run/report.json), a bench median (.bench/report.json), and
    // the serve state -- not figures this file invents. Every slot opens on the
    // honest default (measures.mjs's NOT_WIRED) until something calls
    // showMeasures with what it actually reached; a slot that stays default
    // stays visibly "not wired" rather than drawing a fabricated number.
    const suite = el('span', 'strip-suite', `suite: ${NOT_WIRED}`);
    const bench = el('span', 'strip-bench', `bench: ${NOT_WIRED}`);
    const serve = el('span', 'strip-serve', `served: ${NOT_WIRED}`);
    // [B3] The one live region in this document, and until now there were none: a
    // census of the whole repository found zero `aria-live`, zero `role="status"` and
    // zero `role="alert"`. Every refusal, every undo, every redo and every copy
    // happened silently as far as a screen reader was concerned -- the window told a
    // sighted reader why it would not do a thing and told everyone else nothing.
    //
    // It is separate from `.strip-said` rather than an attribute on it, because the
    // strip is redrawn on every frame and a live region that is rewritten with
    // identical text on each paint announces the same sentence over and over. This one
    // is written only in `announce`, and only when the words actually change.
    const live = el('div', 'strip-live');
    live.setAttribute('aria-live', 'polite');
    live.setAttribute('aria-atomic', 'true');
    live.setAttribute('role', 'status');
    strip.append(digest, said, counts, checks, suite, bench, serve, live);
    this.#parts.stripLive = live;
    this.#parts.strip = strip;
    this.#parts.stripDigest = digest;
    this.#parts.stripSaid = said;
    this.#parts.stripCounts = counts;
    this.#parts.stripChecks = checks;
    this.#parts.stripSuite = suite;
    this.#parts.stripBench = bench;
    this.#parts.stripServe = serve;
  }

  say(row) {
    this.#standing = { said: row.said ?? '', outcome: row.outcome };
    this.announce(row.said ?? '', row.outcome);
  }

  /** Put words into the one live region, and only when they change. A refusal is
   * named as one: "refused" ahead of the reason is the difference between a reader
   * hearing why something did not happen and hearing a sentence with no verdict on
   * it. Repeating identical text is skipped because a polite region re-announces on
   * every write, and the strip is written on every paint. */
  announce(words, outcome) {
    const live = this.#parts.stripLive;
    if (!live) return;
    const text = words === '' ? '' : (outcome === 'refused' ? `refused: ${words}` : words);
    if (live.textContent === text) return;
    live.textContent = text;
  }

  /**
   * @param {object} state
   * @param {{past:number,ahead:number,dropped:number}} depth
   * @param {string} digest the line's digest, computed once by the state and passed in;
   *   the frame does not get to compute its own opinion of what it is showing
   */
  draw(state, depth, digest) {
    // The last state drawn, kept so a control built once (the palette) can ask what is
    // true now rather than closing over what was true when it was built -- the stale
    // closure shape req/103 Finding 1 records as a real data-loss defect on a face.
    this.#lastState = state;
    document.documentElement.setAttribute('data-theme', state.theme);
    this.spaceIndex = state.space;
    const space = state.spaces[state.space];
    this.#spaceNameValue = space.name;
    this.#drawRail(state);
    this.#drawDockActs(state);
    for (const side of DOCK_SIDES) this.#drawDock(side, space);
    this.#syncNode(this.#parts.stage, 0, space.stage, []);
    while (this.#parts.stage.children.length > 1) this.#retire(this.#parts.stage.lastElementChild);
    this.#drawStrip(depth, digest);
    // Last, because it reads what the lines above have just put on the screen. A face
    // is mounted by #syncNode/#drawDock, so on the first paint there is nothing to
    // read until those have run.
    this.#drawCounts(state);
    this.reservePins();
  }

  /**
   * Every count slot in the frame, filled from what is standing.
   *
   * One pass and one mechanism for two consumers -- the sidebar and the tab strip --
   * because two readings of the same fact are two things that can disagree, and the
   * defect req/784 R-07 records as a defect CLASS (not a slip: it survived a whole
   * redesign) is exactly a surface showing two totals for one population with no
   * relation stated between them.
   */
  #drawCounts(state) {
    const census = censusOf(this.#readings());
    for (const slot of this.#parts.shell.querySelectorAll('[data-count-for]')) {
      const id = slot.dataset.countFor;
      const found = census.get(id);
      // Which of the three (COUNT_SAID) this slot is in. `stands` is asked of the state,
      // never inferred from the absence of a number -- inferring it is what made the
      // window contradict itself (req/811 §8-2b).
      const stands = state ? this.#standsAnywhere(state, id) : false;
      const which = found ? COUNT_STATE.READ : (stands ? COUNT_STATE.STANDING : COUNT_STATE.UNPLACED);
      slot.textContent = countText(found);
      slot.dataset.read = String(Boolean(found));
      slot.dataset.count = which;
      slot.title = found ? COUNT_SAID.read(found) : COUNT_SAID[which];
    }
  }

  /**
   * Re-census when a face changes what it is displaying.
   *
   * `#drawCounts` reads what each face has drawn, and `draw()` calls it last for exactly
   * that reason -- but a face answers the engine AFTER it mounts, redraws its own band
   * when the answer arrives, and nothing was asking the shell to look again. So the
   * sidebar held whatever the band said at mount time, forever.
   *
   * Unmeasurable before the membrane was bound, because every band was empty at mount and
   * stayed empty. Measured the day it was bound: the notice face's band read `4 calls`
   * while its own sidebar slot read `1`. That is req/784 R-07's defect class exactly --
   * one population, two readings, no relation stated between them -- and it is the defect
   * this frame's single census pass was built to prevent, arriving through the one door
   * the pass did not watch.
   *
   * One observer, one census per frame. Attribute-filtered to the value a count is read
   * from, so an unrelated repaint inside a face costs nothing.
   */
  #watchCounts(shell) {
    const view = shell.ownerDocument?.defaultView;
    if (typeof view?.MutationObserver !== 'function') return;
    let asked = false;
    const observer = new view.MutationObserver(() => {
      if (asked) return;
      asked = true;
      view.requestAnimationFrame(() => {
        asked = false;
        if (this.#lastState) this.#drawCounts(this.#lastState);
      });
    });
    observer.observe(shell, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: [COUNT_SOURCE.value, COUNT_SOURCE.noun],
    });
    this.#countWatch = observer;
  }

  /**
   * [C4] The engaged pin's measured height, written onto its own scroller as `--pin-h`.
   *
   * The pin (`shell.css` `[data-pin='terminal-act']:has(button:not([disabled]))`) stands
   * over the foot of its scroller, and c3 recorded the debt: the band was unreserved, so
   * an arriving pin was a silent 179px overlay, and c2's attempt to reserve it with a
   * flat 6rem bought a second occlusion because a guessed height is not a measured one.
   * So the height is never written anywhere -- it is read off the engaged pin's own box
   * here, and `.dock-host` turns it into `padding-bottom` and `scroll-padding-bottom`.
   * No engaged pin, no property, no reservation: the empty bed pays nothing.
   *
   * Public and synchronous, because the observer below coalesces through
   * requestAnimationFrame and a check that flips `disabled` needs the reservation to be
   * askable in the same breath rather than a frame later.
   */
  reservePins() {
    for (const host of this.#parts.shell.querySelectorAll('.dock-host')) {
      const pin = [...host.querySelectorAll("[data-pin='terminal-act']")]
        .find((row) => row.querySelector('button[data-act]:not([disabled])'));
      if (pin) host.style.setProperty('--pin-h', `${pin.offsetHeight}px`);
      else host.style.removeProperty('--pin-h');
    }
  }

  /**
   * Re-measure when an act's `disabled` flips or a face redraws its rows -- the two
   * doors a pin engages or leaves through that `draw()` does not see, because a face
   * repaints itself when the engine answers and nothing was asking the shell to look
   * again. Same observer shape, coalescing and reasoning as `#watchCounts` above.
   */
  #watchPins(shell) {
    const view = shell.ownerDocument?.defaultView;
    if (typeof view?.MutationObserver !== 'function') return;
    let asked = false;
    const observer = new view.MutationObserver(() => {
      if (asked) return;
      asked = true;
      view.requestAnimationFrame(() => {
        asked = false;
        this.reservePins();
      });
    });
    observer.observe(shell, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['disabled'],
    });
    this.#pinWatch = observer;
  }

  /** The reaching half: what each standing face has drawn at the head of itself. */
  #readings() {
    const out = [];
    for (const host of this.#parts.shell.querySelectorAll('[data-host]')) {
      const drawn = host.querySelector(COUNT_SOURCE.face);
      if (!drawn) continue;
      const segment = drawn.querySelector(COUNT_SOURCE.band);
      out.push({
        id: drawn.getAttribute('data-face'),
        value: segment?.getAttribute(COUNT_SOURCE.value) ?? null,
        noun: segment?.getAttribute(COUNT_SOURCE.noun) ?? null,
      });
    }
    return out;
  }

  // ---- rail: space nouns only (SS551) ------------------------------------------------

  /** The ceiling W14 already holds for a manifest's faces (RAIL.capacity); a rail
   * of top-level spaces is under the same reasoning -- a nav a person can scan at
   * a glance has a stated size, not an unstated one that happens to be small
   * today. Checked here rather than thrown, because a rail that is one space over
   * is a fact to draw ("8 spaces declared, 7 shown"), not a document to refuse. */
  static RAIL_SPACE_CAPACITY = 7;

  #drawRail(state) {
    const rail = this.#parts.rail;
    const over = state.spaces.length - Frame.RAIL_SPACE_CAPACITY;
    const wanted = state.spaces.slice(0, Frame.RAIL_SPACE_CAPACITY).map((space, index) => ({
      key: `space:${space.name}`,
      label: space.name,
      markName: 'space',
      on: index === state.space,
      press: () => this.#act('space:go', { index }),
    }));
    this.#syncList(rail, wanted, (item) => {
      const made = button('rail-item', item.label, item.label);
      made.dataset.key = item.key;
      made.append(mark(item.markName, MARK_SIZE.rail));
      made.append(el('span', 'rail-label', item.label));
      made.addEventListener('click', () => this.#pressed.get(made)?.());
      return made;
    }, (node, item) => {
      this.#pressed.set(node, item.press);
      node.classList.toggle('on', item.on);
      node.querySelector('.rail-label').textContent = item.label;
      node.setAttribute('aria-pressed', String(item.on));
    });
    rail.dataset.over = String(Math.max(0, over));
  }

  // ---- the standing column is gone (req/867) -----------------------------------------
  //
  // `#drawLauncher` lived here and drew six rows -- one per face, each carrying the face's
  // region, its count, and a press that placed it. It is deleted rather than hidden, and
  // what each of its three jobs does now is written down so that "where did that go" has
  // an answer that is not archaeology:
  //
  //   the name          -- the tab strip already carried it, character for character.
  //                        That was the finding (req/811 §2-2, §2-3; Owner raised it five
  //                        times). Nothing replaces it because nothing was lost.
  //   the region + count -- the palette's own rows carry `box` and the address for every
  //                        face in every space, which is strictly more than one space's
  //                        worth, and the strip carries the window's totals. The census
  //                        pass (`#drawCounts`) is unchanged and now fills the tab slots
  //                        only, which is why the census count drops -- fewer slots
  //                        asking, not fewer facts known.
  //   the placement press -- moved into the palette (`#drawPalette`), which is the whole
  //                        of what Owner #372's floor required here. It is the one job of
  //                        the three that was genuinely unique to this column.

  /** The gx line for wherever a face currently stands, or null when it stands nowhere. */
  #addressOf(state, id) {
    const found = corpusOf(this.#read, state).find((row) => row.id === id && row.address);
    return found ? found.address : null;
  }

  #pressed = new WeakMap();

  /** What each chrome control would offer on a right-click, refreshed every paint. */
  #menuContext = new WeakMap();

  #standsAnywhere(state, id) {
    const space = state.spaces[state.space];
    if (DOCK_SIDES.some((side) => space.docks[side].faces.includes(id))) return true;
    const walk = (node) => (isLeaf(node) ? node.tabs.includes(id) : node.kids.some(walk));
    return walk(space.stage);
  }

  #summon(state, face) {
    if (face.place === 'stage') {
      const path = this.#focusPath();
      this.#act('tab:add', { index: state.space, path, id: face.id });
      return;
    }
    this.#act('dock:add', { index: state.space, side: face.place, id: face.id });
  }

  /** Where the person is looking decides where a summoned face lands. That is a use of
   *  the viewpoint, not a storing of it: nothing here goes onto the line. */
  #focusPath() {
    const focus = this.#view.focus;
    if (typeof focus !== 'string') return [];
    return focus === '' ? [] : focus.split('.').map(Number);
  }

  // ---- docks ------------------------------------------------------------------------

  #drawDock(side, space) {
    const node = this.#parts.docks[side];
    const dock = space.docks[side];
    node.classList.toggle('shut', !dock.open || dock.faces.length === 0);
    node.style.setProperty(side === 'bottom' ? '--dock-h' : '--dock-w', `${dock.size}px`);
    node.dataset.count = String(dock.faces.length);
    node.dataset.capacity = String(DOCK_RULES[side].capacity);

    // A dock holds several faces and shows one. The others are named along its head, so
    // "it is not here" and "it is behind that one" are different things to look at.
    const shown = dock.faces[dock.at] ?? null;
    const wanted = shown === null ? [] : [{ key: `${side}:${shown}`, id: shown, index: dock.at }];
    this.#syncList(node, wanted, (item) => {
      const made = el('section', 'dock-face');
      made.dataset.key = item.key;
      const bar = el('div', 'dock-bar');
      const names = el('div', 'dock-names');
      const shut = button('tab-close', `close ${item.id}`, `take this face out of the ${side} dock`);
      shut.append(mark('close', MARK_SIZE.tab));
      bar.append(names, shut);
      const meta = el('div', 'object-meta');
      meta.append(this.#breadcrumb([]), this.#commandBlock());
      const host = el('div', 'dock-host');
      host.dataset.host = `dock:${side}:${item.id}`;
      made.append(bar, meta, host);
      shut.addEventListener('click', () => this.#act('dock:drop', { index: this.#spaceIndex, side, at: Number(made.dataset.index) }));
      return made;
    }, (made, item) => {
      const face = this.#read.byId.get(item.id);
      made.dataset.index = String(item.index);
      made.dataset.condition = face?.condition ?? 'plain';
      // On the dock, not on the face inside it, and that is the whole of the fix.
      //
      // This line set the variable on the `.dock-face` section, and no rule anywhere
      // read it -- so a face's declared `leastWidth` has been decorative since the field
      // existed. W14's "every declared field has a reader, and the reader reads it"
      // passed on a technicality: this file mentions the field, and mentioning is not
      // reading. Measured in the real app window at 1426px: the right dock drew 190px
      // against a face declaring 320, so that face's stat band collapsed to a column of
      // dashes and its controls wrapped three words to a line. A variable set on a child
      // cannot be read by the parent that sizes it, which is why moving it up is the
      // change and `max()` in shell.css is the rest of it.
      if (face?.leastWidth) node.style.setProperty('--least-w', `${face.leastWidth}px`);
      else node.style.removeProperty('--least-w');
      // The same fix, on the other axis, found the same way. `leastWidth` was decorative
      // until the line above read it; `leastHeight` did not exist at all, so a face put in
      // the bottom dock got whatever the saved layout string said and had no way to say it
      // needed more. Measured in the real app window at 1440x900: the bottom dock stood
      // 121px tall, its own bar and breadcrumb took 83 of them, and the face was handed
      // **38px** to draw 427px of content in -- a dock that is not showing a face so much
      // as proving one is mounted. `leastWidth: 320` on a bottom-docked face was decorative
      // in the second sense too: a bottom dock is already full width, so the only number
      // that face could state was the one nothing sized it by.
      if (face?.leastHeight) node.style.setProperty('--least-h', `${face.leastHeight}px`);
      else node.style.removeProperty('--least-h');
      this.#names(made.querySelector('.dock-names'), dock, side);
      this.#hold(made.querySelector('.dock-host'), face);
      // A dock shows one face at a time, so "the side dock" and "the face in it" land
      // on the same act (dock:go at this item's index) -- unlike the stage, a dock has
      // no separate pane-focus concept for its middle segment to reach instead.
      const toThisDockFace = () => this.#act('dock:go', { index: this.#spaceIndex, side, at: item.index });
      this.#fillBreadcrumb(made.querySelector('.breadcrumb'), [
        { label: space.name, go: () => this.#act('space:go', { index: this.#spaceIndex }) },
        { label: `${side} dock`, go: toThisDockFace },
        { label: face?.title ?? item.id, go: toThisDockFace },
      ]);
      this.#fillCommand(made.querySelector('.command'), commandFor('dock', {
        index: this.#spaceIndex, side, at: item.index, id: item.id,
      }));
    });
  }

  #names(bar, dock, side) {
    const wanted = dock.faces.map((id, index) => ({ key: id, id, index }));
    this.#syncList(bar, wanted, (item) => {
      const made = button('dock-name', item.id, item.id);
      made.dataset.key = item.key;
      made.append(el('span', 'dock-name-label'));
      made.addEventListener('click', () => this.#act('dock:go', { index: this.#spaceIndex, side, at: Number(made.dataset.index) }));
      made.addEventListener('contextmenu', (event) => {
        const held = this.#menuContext.get(made);
        if (held) this.#openMenu(event, 'dock', held);
      });
      return made;
    }, (made, item) => {
      const face = this.#read.byId.get(item.id);
      made.dataset.index = String(item.index);
      made.classList.toggle('on', item.index === dock.at);
      made.setAttribute('aria-pressed', String(item.index === dock.at));
      made.querySelector('.dock-name-label').textContent = face?.title ?? item.id;
      this.#menuContext.set(made, {
        index: this.#spaceIndex, side, at: item.index, id: item.id, size: dock.size,
      });
    });
  }

  // ---- stage ------------------------------------------------------------------------

  #syncNode(parent, index, node, path) {
    let here = parent.children[index];
    const kind = node.k;
    const shapeChanged = here
      && (here.dataset.kind !== kind
        || (kind === 's' && (here.dataset.axis !== node.axis || Number(here.dataset.kids) !== node.kids.length)));
    if (!here || shapeChanged) {
      const made = kind === 's' ? el('div', 'split') : this.#makeLeaf();
      made.dataset.kind = kind;
      // Put the new one in first, then retire the old one. Retiring detaches the node, so
      // the other order asks the parent to replace a child it no longer has -- which the
      // DOM refuses, and which no amount of reading this file would have shown.
      if (here) { parent.insertBefore(made, here); this.#retire(here); } else { parent.append(made); }
      here = made;
    }
    here.dataset.path = path.join('.');
    if (kind === 's') this.#syncSplit(here, node, path); else this.#syncLeaf(here, node, path);
  }

  #syncSplit(node, model, path) {
    node.dataset.axis = model.axis;
    node.dataset.kids = String(model.kids.length);
    node.classList.toggle('down', model.axis === 'col');
    // children are kid, sash, kid, sash, kid ...
    const wanted = model.kids.length * 2 - 1;
    while (node.children.length > wanted) this.#retire(node.lastElementChild);
    let slot = 0;
    model.kids.forEach((kid, i) => {
      if (i > 0) {
        let sash = node.children[slot];
        if (!sash || !sash.classList.contains('sash-pane')) {
          const made = el('div', 'sash sash-pane');
          made.setAttribute('role', 'separator');
          made.addEventListener('pointerdown', (event) => this.#dragPane(event, node, model, path, i - 1));
          if (sash) node.insertBefore(made, sash); else node.append(made);
          sash = made;
        }
        sash.setAttribute('aria-label', `${model.axis === 'row' ? 'column' : 'row'} divider`);
        slot += 1;
      }
      this.#syncNode(node, slot, kid, [...path, i]);
      const child = node.children[slot];
      child.style.flexGrow = String(model.ratios[i]);
      child.style.flexBasis = '0';
      slot += 1;
    });
  }

  #makeLeaf() {
    const made = el('div', 'pane');
    const bar = el('div', 'tabs');
    bar.setAttribute('role', 'tablist');
    const tools = el('div', 'pane-tools');
    for (const [name, verb, axis, said] of [
      ['divide-row', 'pane:divide', 'row', 'divide this pane across'],
      ['divide-col', 'pane:divide', 'col', 'divide this pane down'],
      ['close', 'pane:drop', null, 'drop this pane'],
    ]) {
      const tool = button('pane-tool', said, said);
      tool.append(mark(name, MARK_SIZE.tab));
      tool.addEventListener('click', () => {
        const path = made.dataset.path === '' ? [] : made.dataset.path.split('.').map(Number);
        this.#act(verb, { index: Number(made.dataset.space), path, axis });
      });
      tools.append(tool);
    }
    const meta = el('div', 'object-meta');
    meta.append(this.#breadcrumb([]), this.#commandBlock());
    const host = el('div', 'pane-host');
    made.append(bar, tools, meta, host);
    made.addEventListener('pointerdown', () => this.#look(made.dataset.path));
    return made;
  }

  /**
   * [C3] Looking, and the window saying so.
   *
   * `.pane.looked { border-color: var(--tension) }` has been in `shell.css` the whole
   * time, and the class that turns it on was written in exactly one place: the
   * `classList.toggle('looked', ...)` line inside `#syncLeaf`, which runs on a full
   * draw. `Viewpoint.look` is deliberately not state -- it is not on the layout line and
   * has no act in the registry, so undo cannot reach it -- and the price of that, unpaid
   * until now, is that nothing schedules a draw when it changes. So the focus ring was
   * correct only by accident, on whatever redraw happened next for some other reason.
   *
   * `btn_verify` found it from the other side: the breadcrumb's `stage` crumb was the
   * one `[silent]` finding on the window -- pressed, and nothing in the document changed
   * and no reason was given -- because `look()` assigned a private field and returned.
   * The crumb was not broken. The ring was.
   *
   * This repaints the ring alone rather than asking for a draw: a draw would be a
   * redundant pass over every pane and dock for a value that changes nothing they show,
   * and `changing()` exists precisely so that redraws stay rare.
   */
  #look(where) {
    const now = this.#view.look(where);
    for (const pane of this.#root.querySelectorAll('.pane[data-path]')) {
      pane.classList.toggle('looked', pane.dataset.path === now);
    }
    return now;
  }

  #syncLeaf(node, model, path) {
    node.dataset.space = String(this.#spaceIndex);
    node.classList.toggle('bare', model.tabs.length === 0);
    const bar = node.querySelector('.tabs');
    const wanted = model.tabs.map((id, index) => ({ key: id, id, index }));
    // [C3 / B2] Two acts, two buttons, and the reason they cannot be one node.
    //
    // A tab was a single `button.tab` with a `span.tab-close` inside it, and which of the
    // two acts fired was decided at click time by `event.target.closest('.tab-close')`.
    // Three things were wrong with that and only the first is cosmetic:
    //
    //   1. A control inside a control is not addressable. `btn_verify`'s census is
    //      `button,[role=button],summary,[data-act],[tabindex],a[href],input,select` --
    //      a bare `span` matches none of them, so closing a tab was one of the window's
    //      acts that no gate had ever pressed. It was not failing the census; it was
    //      absent from the denominator, which is worse.
    //   2. There is no keyboard route to it at all. The tab is focusable and Enter fires
    //      its click with `event.target` on the button, so the close branch is
    //      unreachable without a pointer. Every other act on this window has a key.
    //   3. Nesting a button inside a button is invalid HTML, so the fix cannot be to
    //      promote the span in place -- the tab has to stop being the container.
    //
    // So the keyed node is now a slot holding two siblings. `.tab` keeps its class, its
    // `role`, its `.tab-label`, its `.side-count` and its click, because `bound_smoke`,
    // `shoot_all`, `shoot_window` and `checks` all select it by that class and a
    // restructure that quietly retires a selector six tools depend on is a second defect
    // paying for the first. `on` is carried on both the slot (which draws it) and the
    // button (which every existing reader tests).
    this.#syncList(bar, wanted, (item) => {
      const slot = el('div', 'tab-slot');
      slot.dataset.key = item.key;
      const made = button('tab', item.id, item.id);
      made.setAttribute('role', 'tab');
      made.append(el('span', 'tab-label', item.id));
      // A tab names a face, a face names a collection, so a tab carries a count
      // (req/784 A-02). Filled by the one census pass, never computed here.
      const count = el('span', 'side-count', COUNT_UNREAD);
      count.dataset.countFor = item.id;
      made.append(count);
      const shut = button('tab-close', `close ${item.id}`, `close ${item.id}`);
      shut.append(mark('close', MARK_SIZE.tab));
      slot.append(made, shut);
      const here = () => (node.dataset.path === '' ? [] : node.dataset.path.split('.').map(Number));
      made.addEventListener('click', () => this.#act('tab:go', {
        index: this.#spaceIndex, path: here(), at: Number(slot.dataset.index),
      }));
      shut.addEventListener('click', () => this.#act('tab:close', {
        index: this.#spaceIndex, path: here(), at: Number(slot.dataset.index),
      }));
      slot.addEventListener('contextmenu', (event) => {
        const held = this.#menuContext.get(slot);
        if (held) this.#openMenu(event, 'tab', held);
      });
      return slot;
    }, (slot, item) => {
      const face = this.#read.byId.get(item.id);
      const made = slot.querySelector('.tab');
      slot.dataset.index = String(item.index);
      made.dataset.index = String(item.index);
      const on = item.index === model.active;
      slot.classList.toggle('on', on);
      made.classList.toggle('on', on);
      made.setAttribute('aria-selected', String(on));
      slot.dataset.condition = face?.condition ?? 'plain';
      made.dataset.condition = face?.condition ?? 'plain';
      made.querySelector('.tab-label').textContent = face?.title ?? item.id;
      slot.querySelector('.tab-close').setAttribute('aria-label', `close ${face?.title ?? item.id}`);
      this.#menuContext.set(slot, {
        index: this.#spaceIndex, path, at: item.index, id: item.id, active: model.active,
      });
    });

    // req/822_c7 (Owner #367追記1/#395: the sidebar's rows and a lone tab were two
    // pressable spellings of one face). Ruling recorded in req/822_c7 §5: the two
    // organs answer different questions -- the launcher answers WHERE every face
    // stands (region mark + count, including unplaced), a tab strip answers WHICH of
    // several faces this pane shows. A strip holding one tab has no "which" to
    // answer, so it is drawn as the pane's title rather than as a chooser: the count
    // below lets the stylesheet strip the chooser costume (bed, hover invite) off
    // the solo case. The control itself keeps its class, role and click -- six
    // instruments select `.tab`, and an act that works is never removed (#372).
    bar.dataset.count = String(model.tabs.length);

    const host = node.querySelector('.pane-host');
    host.dataset.host = `stage:${path.join('.')}`;
    const id = model.tabs[model.active] ?? null;
    const face = id ? this.#read.byId.get(id) : null;
    this.#hold(host, face);
    node.classList.toggle('looked', this.#view.focus === node.dataset.path);
    // The middle segment lands on this exact pane (the same #view.look a pointerdown
    // on the pane itself already fires), not a generic "the stage" -- a space can hold
    // several stage panes, and "stage" alone would not say which one this crumb is for.
    this.#fillBreadcrumb(node.querySelector('.breadcrumb'), [
      { label: this.#spaceNameValue, go: () => this.#act('space:go', { index: this.#spaceIndex }) },
      { label: 'stage', go: () => this.#look(node.dataset.path) },
      {
        label: face?.title ?? id ?? 'empty pane',
        go: id === null ? null : () => this.#act('tab:go', { index: this.#spaceIndex, path, at: model.active }),
      },
    ]);
    this.#fillCommand(node.querySelector('.command'), id === null ? null : commandFor('stage', {
      index: this.#spaceIndex, path, at: model.active, id,
    }));
  }

  #spaceIndexValue = 0;

  #spaceNameValue = '';

  set spaceIndex(value) { this.#spaceIndexValue = value; }

  get #spaceIndex() { return this.#spaceIndexValue; }

  // ---- faces ------------------------------------------------------------------------

  /** Raise a face only when the place stops holding the one it held. */
  #hold(host, face) {
    const key = host.dataset.host;
    const wanted = face?.id ?? null;
    if (!changing(this.#mounted, key, wanted)) return;
    this.#mounted.lower(key);
    host.replaceChildren();
    if (!wanted) {
      host.append(el('p', 'bare-said', 'Nothing is placed here. That is a state, not a fault.'));
      return;
    }
    // Three outcomes, three sentences. Declared-and-absent, declared-and-broken, and
    // standing are different facts, and a shell that draws the first two the same way has
    // told a person their installation is broken when it is merely empty.
    if (typeof face.mount !== 'function') {
      host.append(el('p', 'face-absent', `${face.title} is declared and not installed.`));
      return;
    }
    try {
      this.#mounted.raise(key, face, host, this.#port, this.#notices);
    } catch (error) {
      host.replaceChildren(el('p', 'face-broke', `${face.id} did not mount: ${error.message}`));
    }
  }

  #retire(node) {
    for (const host of [node, ...node.querySelectorAll('[data-host]')]) {
      if (host.dataset?.host) this.#mounted.lower(host.dataset.host);
    }
    node.remove();
  }

  // ---- lists ------------------------------------------------------------------------

  /** Reuse by key. Nothing is emptied and refilled, so nothing is mounted twice for it. */
  #syncList(parent, wanted, make, tune) {
    const held = new Map();
    for (const child of [...parent.children]) {
      if (child.dataset.key !== undefined) held.set(child.dataset.key, child);
    }
    let index = 0;
    for (const item of wanted) {
      let node = held.get(item.key);
      if (node) held.delete(item.key); else node = make(item);
      if (parent.children[index] !== node) parent.insertBefore(node, parent.children[index] ?? null);
      tune(node, item);
      index += 1;
    }
    for (const node of held.values()) this.#retire(node);
  }

  // ---- object meta: breadcrumb + copyable command block (SS551) ---------------------

  /** A flat, non-nested trail, joined by a literal " / " -- no borrowed typographic
   * character (the source gate `no-borrowed-symbol` in every face applies the same
   * rule to this file's own text, and a chevron glyph would be one more thing to
   * teach the glyph canon for a separator nobody reads as data).
   *
   * req/97 Pass 2's worst defect on this chrome: the trail was plain text, so a
   * viewer could read "verify / stage / Sheet A" but press none of it -- the chain
   * the rubric asks to be traceable "forward and back" only went forward (tab:go /
   * dock:go land you on an object; nothing landed you back on its container from
   * the crumb itself). Each crumb below is now a `{ label, go }` pair: `go` is the
   * same kind of act closure the rail/launcher/dock-name/tab controls already fire
   * (SS551), called at click time so it reads current state rather than a value
   * snapshotted at render time. A segment with nothing to land on (there is no
   * face here to go back to) passes `go: null` and renders as an honestly-disabled
   * button (the button[disabled] convention every other control in this file
   * already uses), never a control that looks pressable and silently does nothing. */
  #breadcrumb(crumbs) {
    const nav = el('nav', 'breadcrumb');
    nav.setAttribute('aria-label', 'where this is');
    this.#fillBreadcrumb(nav, crumbs);
    return nav;
  }

  #fillBreadcrumb(nav, crumbs) {
    if (!nav) return;
    nav.replaceChildren();
    crumbs.forEach(({ label, go }, index) => {
      if (index > 0) nav.append(el('span', 'crumb-sep', ' / '));
      const made = button('crumb', String(label), `go to ${label}`);
      made.textContent = String(label);
      if (typeof go === 'function') made.addEventListener('click', go);
      else made.disabled = true;
      nav.append(made);
    });
  }

  /** "the gx verb that reproduces what is shown" (SS551), with a control to take
   * it. The text is drawn even when copying fails silently in a sandboxed
   * document (`data-copied`/`data-copy-failed` say which, rather than a control
   * that looks the same whether or not it did anything). */
  #commandBlock() {
    const made = el('div', 'command');
    const code = el('code', 'command-text');
    // req/822_c7 S3 (Owner #388 冗長文字): the echo was drawn inline in every pane
    // header and TRUNCATED there -- a cut command is not a reproduction command, it is
    // noise wearing one's clothes. The full text stays in the DOM (the `code` node,
    // hidden -- N-4's "full value present on the same screen" is now actually true,
    // where the visible echo was the clipped half) and on the copy control's own
    // title; the one thing the header shows is the control a hand can use.
    code.hidden = true;
    const copy = button('command-copy', 'copy command', 'copy the gx command that reproduces this view');
    copy.append(mark('copy', MARK_SIZE.tab));
    copy.addEventListener('click', () => this.#copyCommand(made, code.textContent));
    made.append(code, copy);
    return made;
  }

  #fillCommand(node, text) {
    if (!node) return;
    const code = node.querySelector('.command-text');
    const copy = node.querySelector('.command-copy');
    if (text === null || text === undefined) {
      code.textContent = 'nothing is placed here to reproduce';
      copy.disabled = true;
      copy.title = 'nothing is placed here to reproduce';
      return;
    }
    code.textContent = text;
    copy.disabled = false;
    copy.title = `copy the gx command that reproduces this view -- ${text}`;
  }

  #copyCommand(made, text) {
    made.removeAttribute('data-copied');
    made.removeAttribute('data-copy-failed');
    const clip = typeof navigator !== 'undefined' ? navigator.clipboard : undefined;
    if (!clip || typeof clip.writeText !== 'function') {
      made.setAttribute('data-copy-failed', 'true');
      this.#saidCopy('this window could not reach a clipboard, so the command was not taken', 'refused');
      return;
    }
    clip.writeText(text).then(
      () => {
        made.setAttribute('data-copied', 'true');
        // [B4] The attribute drove a colour change on one icon and nothing else --
        // the same shade of "something happened" whether or not anything had. The
        // words are what confirm it, and they are said where outcomes are said.
        this.#saidCopy('the command was taken, and is on the clipboard', 'moved');
      },
      () => {
        made.setAttribute('data-copy-failed', 'true');
        this.#saidCopy('the clipboard refused, so the command was not taken', 'refused');
      },
    );
  }

  // ---- dragging ---------------------------------------------------------------------

  #dragDock(event, side) {
    const dock = this.#parts.docks[side];
    const rule = DOCK_RULES[side];
    const from = side === 'bottom' ? event.clientY : event.clientX;
    // [B5] The origin was read from `--dock-w` / `--dock-h`, but the dock is drawn at
    // `max(--dock-w, --least-w)` -- the face's declared minimum can be the larger of
    // the two, and then the stored number is not where the sash is. On the right dock
    // a face declaring 320px against a stored floor of 220px put the grip 100px away
    // from the size the arithmetic started at, so the first 100px of every outward
    // drag moved the pointer and nothing else. Measuring the box asks where the sash
    // actually is instead of where the state believes it is.
    const box = dock.getBoundingClientRect();
    const was = Math.round(side === 'bottom' ? box.height : box.width)
      || Number.parseInt(getComputedStyle(dock).getPropertyValue(side === 'bottom' ? '--dock-h' : '--dock-w'), 10)
      || rule.least;
    const sign = side === 'right' || side === 'bottom' ? -1 : 1;
    this.#drag(event, (now) => {
      const moved = (side === 'bottom' ? now.clientY : now.clientX) - from;
      const size = Math.round(Math.min(rule.most, Math.max(rule.least, was + sign * moved)));
      this.#act('dock:size', { index: this.#spaceIndex, side, size });
    });
  }

  #dragPane(event, node, model, path, edge) {
    const across = model.axis === 'row';
    const box = node.getBoundingClientRect();
    const span = across ? box.width : box.height;
    const from = across ? event.clientX : event.clientY;
    const was = [...model.ratios];
    this.#drag(event, (now) => {
      const moved = ((across ? now.clientX : now.clientY) - from) / (span || 1);
      const ratios = [...was];
      ratios[edge] = was[edge] + moved;
      ratios[edge + 1] = was[edge + 1] - moved;
      if (ratios[edge] <= 0 || ratios[edge + 1] <= 0) return;
      this.#act('pane:ratio', { index: this.#spaceIndex, path, ratios });
    });
  }

  #drag(event, move) {
    event.preventDefault();
    const target = event.currentTarget;
    target.classList.add('held');
    const onMove = (now) => move(now);
    const onUp = () => {
      target.classList.remove('held');
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }

  // ---- strip ------------------------------------------------------------------------

  /** Undo and redo from a pointer, through the same pair the keyboard uses, so the two
   * routes cannot come to disagree about what a step is. */
  #step(which) {
    if (!this.#history) return;
    this.#history[which]?.();
  }

  #drawStrip(depth, digest) {
    this.#parts.stripDigest.textContent = `${shortDigest(digest)}`;
    this.#parts.stripDigest.title = digest;
    // The count is the label. A step with nothing behind it is drawn dead rather than
    // drawn live and doing nothing when pressed -- `btn_verify` calls the second one
    // `silent`, and it is right to.
    const steps = [
      [this.#parts.stripUndo, depth.past, 'behind', 'undo'],
      [this.#parts.stripRedo, depth.ahead, 'ahead', 'redo'],
    ];
    for (const [control, count, word, verb] of steps) {
      if (!control) continue;
      // req/822_c7 S5: the count is still the label, the direction word moved to the
      // spoken/hover layer -- `undo 1` reads at a glance where `undo 1 behind` was
      // three words saying one fact twice (the verb already carries the direction).
      control.textContent = `${verb} ${count}`;
      const dead = count === 0 || this.#history === null;
      control.disabled = dead;
      control.setAttribute('aria-label', `${verb}, ${count} ${word}`);
      control.title = dead
        ? `nothing is ${word}, so there is nothing to ${verb}`
        : `${verb} the last ${verb === 'undo' ? '' : 'undone '}shell act (${count} ${word})`;
    }
    if (this.#parts.stripDropped) {
      // req/822_c7 S5: a zero here was standing text on every session's footer. The
      // count is still measured on every draw; it takes room only when it is news.
      this.#parts.stripDropped.textContent = depth.dropped > 0 ? `${depth.dropped} dropped` : '';
      this.#parts.stripDropped.title = 'acts this window let go of when the history reached its depth. There is no act that brings them back';
    }
    this.#parts.stripSaid.textContent = this.#standing.said;
    this.#parts.stripSaid.dataset.outcome = this.#standing.outcome;
    // The full sentence, for the case where the drawn one is still clipped.
    this.#parts.stripSaid.title = this.#standing.said;
  }

  showChecks(text, ok, title = null) {
    this.#parts.stripChecks.textContent = text;
    this.#parts.stripChecks.dataset.ok = String(ok);
    // req/822_c7 S5: the compact figure is what the strip has room for; the full
    // sentence (what mounted, what drew, what is declared) rides its title.
    if (title) this.#parts.stripChecks.title = title;
  }

  /**
   * SS551's live status-bar numbers. Takes the already-formatted shape
   * `measures.mjs`'s `formatMeasures()` returns -- this method draws it, it does
   * not fetch it, for the same reason `draw()` is handed a digest rather than
   * computing its own opinion of one (line 118 above).
   * @param {{suite: {text:string,ok:boolean|null}, bench: object, serve: object}} measures
   */
  showMeasures(measures) {
    for (const [key, node] of [['suite', this.#parts.stripSuite], ['bench', this.#parts.stripBench], ['serve', this.#parts.stripServe]]) {
      const reading = measures?.[key];
      if (!reading) continue;
      node.textContent = reading.text;
      node.dataset.ok = String(reading.ok);
      // req/822_c7 S5: a reading may carry its fuller sentence on the hover layer.
      node.title = reading.title ?? reading.text;
      // req/822_c5 item 1: a suite reading that describes another tree carries the stale
      // mark ahead of its counts, so the eye is told before the numbers are read. The
      // mark is structure (kernel marks.mjs), never a typed character; the dataset bit is
      // for instruments, which should not have to parse a sentence to learn a boolean.
      if (key === 'suite') {
        node.dataset.stale = String(reading.stale ?? null);
        if (reading.stale === true) node.prepend(mark('stale', 10));
      }
    }
  }
}
