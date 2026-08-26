// SPDX-License-Identifier: Apache-2.0
// Checks that run in the window, because a check that runs beside the window is a check
// on a file.
//
// The system this shell was derived from had twenty-nine checks over a face that was
// dead: a missing closing brace meant the whole thing was a syntax error, and not one
// check noticed, because every one of them read the file and none of them put it on a
// screen. So everything here reads the live document after the shell has mounted, and
// several of these deliberately perform acts and put the shell back afterwards -- the
// last check is that it went back.

import { walkOnce, seeded, WALK_SEED, WALK_STEPS } from '../tools/walk.mjs';

const BANNED = ['●', '◆', '◇', '◈', '▾', '▴', '★', '■', '⏺'];

const luminance = (text) => {
  const [r, g, b] = text.match(/[\d.]+/g).slice(0, 3).map(Number);
  const channel = (v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
};

const contrast = (ink, ground) => {
  const a = luminance(ink);
  const b = luminance(ground);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
};

export function run(shell, root) {
  const results = [];
  const check = (name, fn) => {
    try {
      const said = fn();
      results.push({ name, ok: true, said: said ?? '' });
    } catch (error) {
      results.push({ name, ok: false, said: error.message });
    }
  };
  const must = (condition, said) => { if (!condition) throw new Error(said); };

  const opening = shell.digest;
  const doc = root.ownerDocument;

  check('the frame is in the document', () => {
    must(root.querySelector('.shell'), 'no .shell element');
    must(root.querySelector('.rail'), 'no rail');
    must(root.querySelector('.stage'), 'no stage');
    must(root.querySelector('.strip'), 'no strip');
    return 'rail, stage and strip stand';
  });

  check('every placed face mounted, and has elements on the screen', () => {
    const hosts = [...root.querySelectorAll('[data-host]')];
    const carrying = hosts.filter((h) => shell.mounted.idAt(h.dataset.host));
    must(carrying.length > 0, 'nothing is mounted anywhere');
    for (const host of carrying) {
      must(host.querySelector('.demo-face'), `${host.dataset.host} holds a face that drew nothing`);
      must(host.textContent.trim().length > 0, `${host.dataset.host} is empty after mounting`);
    }
    return `${carrying.length} of ${hosts.length} places carry a face`;
  });

  check('no mount was refused', () => {
    const tally = shell.mounted.tally;
    must(tally.refused === 0, `${tally.refused} mounts were refused`);
    must(tally.mounted >= 1, 'nothing was ever mounted');
    return `${tally.mounted} raised, ${tally.unmounted} lowered, 0 refused`;
  });

  check('SS551: the rail carries space nouns only, and nothing else', () => {
    const items = [...root.querySelectorAll('.rail-item')];
    const faceItems = items.filter((n) => !n.dataset.key.startsWith('space:'));
    must(faceItems.length === 0, `${faceItems.length} non-space items are drawn in the rail (keys: ${faceItems.map((n) => n.dataset.key).join(', ')})`);
    must(items.length === shell.state.spaces.length, `${items.length} rail items drawn for ${shell.state.spaces.length} declared spaces`);
    must(items.length <= 7, `the rail draws ${items.length} spaces, past its stated ceiling of 7`);
    return `${items.length} space(s) on the rail, 0 faces`;
  });

  // req/867 (Owner #416追記3). Three checks stood here and all three asserted the sidebar
  // into existence -- that the launcher held every rail-declared face, that the rail and
  // the launcher shared one left edge and one width, and that a control narrowed them
  // both. They are replaced rather than deleted, by the checks that hold the RULING: the
  // region is gone, its roles are where the derivation put them, and nothing vestigial is
  // left behind. An instrument that only knows how to confirm the old shape is an
  // instrument that will report the new one as broken.
  check('req/867: there is no left sidebar, and no vestige of one', () => {
    for (const gone of ['.sidebar', '.launcher', '.launcher-item', '.side-toggle', '.rail-label-head + .launcher']) {
      must(root.querySelectorAll(gone).length === 0, `${gone} is still drawn`);
    }
    // Not merely absent from the markup -- absent from the geometry. A rail collapsed to
    // 0px is still a rail, and "no vestigial rail" is the part of the ruling that a
    // querySelector alone cannot check.
    //
    // Measured on `.middle`, not on `.stage`, and the difference is the whole point of the
    // ruling. `.middle` is the grid area the sidebar used to sit beside, so if it reaches
    // x=0 there is no chrome column left of it. `.stage` does NOT reach x=0 whenever the
    // LEFT DOCK is open -- and on this page it is, holding margin-a at 204px. The first
    // draft of this check asserted on `.stage` and duly failed, calling a legitimately
    // docked face a vestigial sidebar. That is the same instrument-reads-a-ruling-as-a-
    // defect shape this file already carries two notes about, caught this time by the
    // check being run rather than by being assumed to pass. The left dock is exactly the
    // persistent left region req/867's derivation says already exists, so an assertion
    // that forbids it would forbid the thing the ruling points at.
    const shellBox = root.querySelector('.shell').getBoundingClientRect();
    const middle = root.querySelector('.middle').getBoundingClientRect();
    must(middle.left - shellBox.left < 2, `the docks-and-stage region starts ${(middle.left - shellBox.left).toFixed(0)}px in from the window's left edge, so chrome still occupies it`);
    const dock = root.querySelector('.dock-left')?.getBoundingClientRect();
    const said = dock && dock.width > 1 ? `, with the left DOCK (a face, not chrome) at ${dock.width.toFixed(0)}px` : '';
    return `no sidebar; the docks-and-stage region reaches the window's left edge${said}`;
  });

  check('req/867: the window bar spans the window, and carries the SPACE mode, find and theme', () => {
    const bar = root.querySelector('.topbar');
    must(bar, 'no window bar is drawn');
    const box = bar.getBoundingClientRect();
    const shellBox = root.querySelector('.shell').getBoundingClientRect();
    must(Math.abs(box.width - shellBox.width) < 2, `the bar is ${box.width.toFixed(0)}px across a ${shellBox.width.toFixed(0)}px window; a global control has to span what it modifies`);
    must(box.top - shellBox.top < 2, 'the bar is not at the top of the window');
    // The rail is IN the bar, not merely near it -- containment, so a future layout that
    // floats one away from the other is caught here rather than by eye.
    const rail = root.querySelector('.rail');
    must(bar.contains(rail), 'the SPACE rail is not inside the window bar');
    must(bar.contains(root.querySelector('.bar-find')), 'find is not on the window bar');
    must(bar.contains(root.querySelector('.chrome-acts')), 'the chrome acts are not on the window bar');
    // req/811 B-6 held through the move: chrome acts stay outside every nav landmark.
    must(!root.querySelector('nav')?.contains(root.querySelector('.chrome-acts')), 'a chrome act sits inside a nav landmark');
    const label = root.querySelector('.rail-label-head');
    must(label && /\w/.test(label.textContent), 'the mode group carries no label, so two bare verbs read as things to press');
    return `bar ${box.width.toFixed(0)}x${box.height.toFixed(0)}, carrying "${label.textContent.trim()}" + rail + find + acts`;
  });

  check('req/784 A-02: a destination carries a count if and only if it names a collection, and a dash is never a zero', () => {
    // A face names a collection of records and carries a count. A space is a view and
    // carries none. `theme` is an act and carries none.
    // req/867: the face destinations are the tabs now, because the standing column that
    // used to be the other half of this population is gone. The RULE is untouched and is
    // the reason this check survives the removal at all: a destination carries a count if
    // and only if it names a collection. What changed is which destinations exist.
    const faceItems = [...root.querySelectorAll('.tab')];
    must(faceItems.length > 0, 'no face destinations are drawn');
    const without = faceItems.filter((n) => !n.querySelector('.side-count'));
    must(without.length === 0, `${without.length} face destinations carry no count slot`);
    const spaceCounts = [...root.querySelectorAll('.rail-item .side-count')];
    must(spaceCounts.length === 0, `${spaceCounts.length} spaces carry a count, and a space is a view rather than a collection`);
    // [C4] `theme` is not a nav row: req/811 B-6 moved it out of the nav landmark into
    // `.chrome-acts`, and req/867 carried it onto the window bar. This line used to look
    // for it among the `.launcher-item`s, so when the B-6 move landed, `find()` came back
    // undefined and the check failed with a sentence about a count on a control that was
    // not there at all -- an instrument reading a product ruling as a defect (pre-existing
    // at 2057749, unreported by two rounds). The rule it protects is unchanged: wherever
    // the theme act stands, it names no collection and so carries no count.
    const themeInNav = [...root.querySelectorAll('nav .chrome-act, .rail-item')].find((n) => n.dataset.key === 'theme');
    must(!themeInNav, 'the theme act is drawn inside a nav landmark, and req/811 B-6 moved it out');
    const themeAct = [...root.querySelectorAll('.chrome-act')].find((n) => n.querySelector('.chrome-act-label')?.textContent.trim() === 'theme');
    must(themeAct, 'no theme control is drawn anywhere');
    must(!themeAct.querySelector('.side-count'), 'the theme control carries a count, and it names no collection');
    const tabs = faceItems;
    const tabsWithout = without;

    // Every slot is either a number a face is drawing right now, or the dash. Nothing
    // in between, and in particular never a fabricated zero: these demo faces draw no
    // band at all, so on this page every slot must honestly read unread.
    const slots = [...root.querySelectorAll('.side-count')];
    const lying = slots.filter((n) => n.dataset.read === 'false' && n.textContent.trim() !== '--');
    must(lying.length === 0, `${lying.length} slots say something other than a dash while having read nothing`);
    const read = slots.filter((n) => n.dataset.read === 'true');
    for (const slot of read) must(/^\d+$/.test(slot.textContent.trim()), `a slot that claims to have read something says "${slot.textContent}"`);
    return `${faceItems.length} faces and ${tabs.length} tabs counted, ${spaceCounts.length} spaces counted, ${read.length} of ${slots.length} slots have a reading`;
  });

  check('Owner #340 (5): every control that acts says so under a pointer', () => {
    // A solo tab is excluded, and it is a ruling rather than an exemption. req/822_c7
    // (Owner #395-1) made the only tab in a pane that pane's TITLE -- `.tabs[data-count='1']
    // .tab` is drawn at cursor:default on purpose, because a control that switches to the
    // thing already shown is not a control. This check has been failing on it ever since
    // (measured: 1 of 31 on the tree before this lane, with the same `tab on` in its
    // message), which is the third instance in this file of an instrument reporting a
    // product ruling as a defect. It is corrected here rather than left, because a red
    // this lane's own work would otherwise be blamed for is worse than a fix outside its
    // strict scope. When a pane holds more than one tab, every tab is a control again and
    // is measured again.
    const soloTab = (n) => n.closest('.tabs')?.dataset.count === '1';
    const acting = [...root.querySelectorAll('.rail-item, .chrome-act, .tab, .dock-name, .command-copy, .crumb:not([disabled])')]
      .filter((n) => !(n.classList.contains('tab') && soloTab(n)));
    must(acting.length > 0, 'no controls to measure');
    const silent = acting.filter((n) => getComputedStyle(n).cursor !== 'pointer');
    must(silent.length === 0, `${silent.length} of ${acting.length} controls draw no pointer: ${silent.slice(0, 4).map((n) => n.className).join(', ')}`);
    const refused = [...root.querySelectorAll('.crumb[disabled]')];
    const wrong = refused.filter((n) => getComputedStyle(n).cursor === 'pointer');
    must(wrong.length === 0, `${wrong.length} disabled controls invite a press anyway`);
    return `${acting.length} controls invite a press, ${refused.length} disabled ones do not`;
  });

  check('every mark was drawn at a stated size', () => {
    const marks = [...root.querySelectorAll('svg.mark')];
    must(marks.length > 0, 'no marks were drawn at all');
    const unsized = marks.filter((m) => !m.getAttribute('width') || !m.getAttribute('height'));
    must(unsized.length === 0, `${unsized.length} marks have no width or height and fall back to 300x150`);
    const unnamed = marks.filter((m) => m.classList.contains('mark-unnamed'));
    must(unnamed.length === 0, `${unnamed.length} marks were asked for by a name nobody drew`);
    return `${marks.length} marks, all sized`;
  });

  check('SS551: every object view carries a breadcrumb and a copyable command block', () => {
    const metas = [...root.querySelectorAll('.object-meta')];
    must(metas.length > 0, 'no .object-meta was drawn anywhere');
    for (const meta of metas) {
      const crumb = meta.querySelector('.breadcrumb');
      must(crumb, 'an object-meta block carries no breadcrumb');
      must(crumb.querySelectorAll('.crumb').length >= 2, `a breadcrumb has fewer than 2 crumbs: "${crumb.textContent}"`);
      const command = meta.querySelector('.command');
      must(command, 'an object-meta block carries no command');
      const text = command.querySelector('.command-text');
      must(text && text.textContent.trim().length > 0, 'a command block draws an empty command line');
      const copy = command.querySelector('.command-copy');
      must(copy, 'a command block carries no copy control');
    }
    const withGx = metas.filter((m) => m.querySelector('.command-text').textContent.startsWith('gx '));
    must(withGx.length > 0, 'no command block reproduces a gx verb -- every one reads the bare-pane fallback');
    return `${metas.length} object views, ${withGx.length} with a live gx command`;
  });

  check('req/97 Pass 2 worst defect: every breadcrumb crumb is a control that lands on its own object', () => {
    const before = shell.digest;
    const beforeDepth = shell.depth;
    const crumbs = [...root.querySelectorAll('.crumb')];
    must(crumbs.length > 0, 'no .crumb controls are drawn anywhere');
    const enabled = crumbs.filter((c) => !c.disabled);
    must(enabled.length > 0, 'every crumb is disabled -- nothing on the screen can be pressed back to');

    // The stage pane's middle ("stage") crumb re-focuses this exact pane -- the same
    // #view.look a pointerdown on the pane body already fires (SS551) -- readable
    // straight off the viewpoint even though it is not state and does not repaint
    // on its own (same as that pointerdown). It is the one segment whose effect
    // needs no second object to navigate to: a pane the checks have not yet
    // clicked starts unfocused, so before/after are provably different.
    const pane = root.querySelector('.pane');
    must(pane, 'no stage pane to read a breadcrumb from');
    const paneCrumbs = pane.querySelector('.breadcrumb').querySelectorAll('.crumb');
    must(paneCrumbs.length === 3, `the stage pane's breadcrumb has ${paneCrumbs.length} crumbs, not the 3 (space / stage / face) SS551 draws`);
    const focusBefore = shell.viewpoint.focus;
    paneCrumbs[1].click();
    const focusAfter = shell.viewpoint.focus;
    must(focusAfter === pane.dataset.path, `pressing the "stage" crumb left the viewpoint at ${JSON.stringify(focusAfter)}, not this pane's own path ${JSON.stringify(pane.dataset.path)}`);
    must(focusAfter !== focusBefore, 'the viewpoint read the same before and after the press, so this proves nothing moved');

    // Every other enabled crumb currently names the object the reader is already
    // looking at (SS551's trail reads "where this is" from here) -- so pressing it
    // fires `space:go`/`dock:go`/`tab:go` at the index already showing, which the
    // act registry itself treats as a clean no-op (acts.mjs: `if (... === state
    // ...) return null` on all three), never a history entry. Pressed here to
    // prove the click reaches a real act rather than a handler that silently does
    // nothing -- the exact shape of req/97's worst defect (a control that looks
    // pressable and was not wired to anything).
    for (const c of enabled) c.click();
    must(shell.digest === before, `pressing every crumb moved the shell from ${before.slice(0, 12)}... to ${shell.digest.slice(0, 12)}...`);
    must(shell.depth.past === beforeDepth.past, `pressing every crumb left ${shell.depth.past - beforeDepth.past} extra history entries behind`);
    return `${crumbs.length} crumbs (${enabled.length} enabled) pressed; the stage crumb moved the viewpoint to "${focusAfter}"; shell digest unmoved`;
  });

  check('SS553: the new command-copy controls meet the 36px tap floor', () => {
    const buttons = [...root.querySelectorAll('.command-copy')].filter((b) => !b.disabled);
    must(buttons.length > 0, 'no enabled command-copy control exists to measure');
    const short = buttons.map((b) => b.getBoundingClientRect()).filter((r) => r.width < 36 || r.height < 36);
    must(short.length === 0, `${short.length} of ${buttons.length} command-copy controls are under 36x36px`);
    return `${buttons.length} command-copy controls, all >= 36x36px`;
  });

  check('SS551: the strip carries a suite, a bench and a serve slot, each honest', () => {
    for (const cls of ['.strip-suite', '.strip-bench', '.strip-serve']) {
      const node = root.querySelector(cls);
      must(node, `no ${cls} element is drawn on the strip`);
      must(node.textContent.trim().length > 0, `${cls} is drawn empty`);
      must('ok' in node.dataset, `${cls} carries no data-ok, so a reader cannot tell measured from not-wired`);
    }
    return [...root.querySelectorAll('.strip-suite, .strip-bench, .strip-serve')].map((n) => n.textContent).join(' | ');
  });

  check('SS558: body text is drawn at 14px or larger everywhere the shell itself draws text', () => {
    const px = (node) => Number.parseFloat(getComputedStyle(node).fontSize);
    const offenders = [];
    for (const node of [
      ...root.querySelectorAll('.rail-label, .rail-label-head, .chrome-act-label, .dock-name, .tab-label, .breadcrumb, .command-text, .strip, .strip-suite, .strip-bench, .strip-serve, .bare-said'),
    ]) {
      const size = px(node);
      if (Number.isFinite(size) && size > 0 && size < 14) offenders.push(`${node.className}: ${size}px`);
    }
    must(offenders.length === 0, `${offenders.length} elements are under the 14px floor: ${offenders.slice(0, 5).join(', ')}`);
    return `checked, 0 under 14px`;
  });

  check('no general-purpose symbol reached the screen', () => {
    const text = root.textContent ?? '';
    const found = BANNED.filter((ch) => text.includes(ch));
    must(found.length === 0, `the screen carries ${found.join(' ')}`);
    return `${BANNED.length} symbols looked for, none found`;
  });

  check('light is the ground state', () => {
    must(doc.documentElement.getAttribute('data-theme') === 'light', 'the shell did not open in light');
    return 'opened light';
  });

  check('both themes are readable', () => {
    const said = [];
    for (const theme of ['light', 'dark']) {
      shell.act('theme:set', { theme });
      const strip = root.querySelector('.strip');
      const seen = getComputedStyle(strip);
      const ground = getComputedStyle(root.querySelector('.shell')).backgroundColor;
      must(!ground.includes('rgba(0, 0, 0, 0)'), `${theme}: the frame paints no ground of its own`);
      const ratio = contrast(seen.color, ground);
      must(ratio >= 4.5, `${theme}: the strip reads at ${ratio.toFixed(2)}:1 against the frame`);
      said.push(`${theme} ${ratio.toFixed(1)}:1`);
    }
    shell.act('theme:set', { theme: 'light' });
    return said.join(', ');
  });

  check('a placement nobody declared is refused in words', () => {
    const onStage = shell.read.faces.find((f) => f.place === 'stage');
    const row = shell.act('dock:add', { index: shell.state.space, side: 'left', id: onStage.id });
    must(row.outcome === 'refused', `placing a stage face in the left dock came back "${row.outcome}"`);
    must((row.said ?? '').length > 10, 'the refusal gave no reason');
    must(row.before === row.after, 'a refused act moved the shell');
    return row.said;
  });

  check('an act on the face track says its inverse is elsewhere', () => {
    const row = shell.act('record:undo', {});
    must(row.outcome === 'elsewhere', `the face-track act came back "${row.outcome}"`);
    must(row.before === row.after, 'a delegated act moved the shell');
    return row.said.slice(0, 60);
  });

  check('a face reached the membrane only through the watch', () => {
    const asked = shell.notices.filter((n) => n.through === 'shell' && n.outcome === 'asked');
    must(asked.length > 0, 'no face called the port, so the watch proved nothing');
    return `${asked.length} calls written down`;
  });

  check('every declared face has stood in this window at least once', () => {
    // Not "every face's file parses". Each one is brought into view, mounted for real,
    // and then read back off the document -- because a face can be declared, imported,
    // syntactically perfect and still draw nothing, and the only thing that tells them
    // apart is putting it on a screen and looking at what is there.
    const index = shell.state.space;
    const stood = [];
    let moved = 0;
    const bring = (verb, args) => { if (shell.act(verb, args).outcome === 'moved') moved += 1; };
    const walk = (id) => {
      const into = (node, path) => {
        if (node.k === 'l') return node.tabs.includes(id) ? { path, at: node.tabs.indexOf(id) } : null;
        for (let i = 0; i < node.kids.length; i += 1) {
          const found = into(node.kids[i], [...path, i]);
          if (found) return found;
        }
        return null;
      };
      return into(shell.state.spaces[index].stage, []);
    };
    const firstLeaf = (node, path = []) => (node.k === 'l' ? path : firstLeaf(node.kids[0], [...path, 0]));

    // [C4] The undos run in a finally, because a check that performs acts and asserts
    // between them restores the shell on the failing path too -- before this, one thrown
    // `must` here left every act it had already performed on the line, and "the shell is
    // where it opened" then reported the wreckage as its own separate failure.
    try {
      for (const face of shell.read.faces) {
        if (face.place === 'stage') {
          // req/811 §8-6: the stage opens on ONE tab, so on arrival most stage faces
          // stand nowhere -- by ruling, not by defect. This check used to demand they
          // all stand, which is the pre-§8-6 world (pre-existing failure at 2057749,
          // unreported by two rounds). Standing one is a single press in the palette
          // (`#summon` fires exactly this verb, and req/867 made the palette's own
          // nowhere-row its second caller), so the check does what the press does, and
          // the finally takes it down again.
          let where = walk(face.id);
          if (!where) {
            bring('tab:add', { index, path: firstLeaf(shell.state.spaces[index].stage), id: face.id });
            where = walk(face.id);
          }
          must(where, `${face.id} declares the stage and cannot be stood in any pane`);
          bring('tab:go', { index, path: where.path, at: where.at });
          const host = root.querySelector(`[data-host="stage:${where.path.join('.')}"]`);
          must(shell.mounted.idAt(host.dataset.host) === face.id, `${face.id} was brought forward and something else is mounted`);
          must(host.querySelector('.demo-face'), `${face.id} mounted and drew nothing`);
          stood.push({ id: face.id, where: host.dataset.host, drew: host.textContent.trim().length });
          continue;
        }
        const dock = shell.state.spaces[index].docks[face.place];
        const at = dock.faces.indexOf(face.id);
        must(at >= 0, `${face.id} declares the ${face.place} dock and does not stand in it`);
        bring('dock:go', { index, side: face.place, at });
        const key = `dock:${face.place}:${face.id}`;
        const host = root.querySelector(`[data-host="${key}"]`);
        must(host, `${face.id} was brought forward and its host is not in the document`);
        must(shell.mounted.idAt(key) === face.id, `${face.id} was brought forward and is not mounted`);
        must(host.querySelector('.demo-face'), `${face.id} mounted and drew nothing`);
        stood.push({ id: face.id, where: key, drew: host.textContent.trim().length });
      }

      document.documentElement.dataset.stood = JSON.stringify(stood);
      must(stood.length === shell.read.faces.length, `${stood.length} of ${shell.read.faces.length} faces stood`);
      return `${stood.length} faces mounted and drew something`;
    } finally {
      for (let i = 0; i < moved; i += 1) shell.undo();
    }
  });

  check('switching a tab does not disturb another pane', () => {
    // [C4] req/811 §8-6 opens the stage on ONE tab, so the bed this check needs -- a
    // pane with two tabs beside a pane holding one -- does not exist on arrival any
    // more, and demanding it was the pre-§8-6 world (pre-existing failure at 2057749,
    // unreported by two rounds). The bed is built here from the shell's own placement
    // verb, and the finally takes every step down again -- on the failing path too,
    // which the three bare undos at the end of the old body never did.
    const spaceIndex = shell.state.space;
    let moved = 0;
    const bring = (verb, args) => { if (shell.act(verb, args).outcome === 'moved') moved += 1; };
    try {
      const standing = (id) => {
        const into = (n) => (n.k === 'l' ? n.tabs.includes(id) : n.kids.some(into));
        return into(shell.state.spaces[spaceIndex].stage);
      };
      for (const face of shell.read.faces.filter((f) => f.place === 'stage')) {
        if (!standing(face.id)) bring('tab:add', { index: spaceIndex, path: [], id: face.id });
      }
      bring('pane:divide', { index: spaceIndex, path: [], axis: 'row' });
      bring('tab:move', { index: spaceIndex, from: [0], at: 0, to: [1] });
      const other = shell.mounted.idAt('stage:1');
      must(other !== null, 'the second pane holds nothing, so this check would pass without measuring anything');
      const before = shell.mounted.tally.unmounted;
      const leaf = shell.state.spaces[spaceIndex].stage.kids[0];
      must(leaf.tabs.length >= 2, `the first pane carries ${leaf.tabs.length} tab(s), not the two this check just placed`);
      const to = leaf.active === 0 ? 1 : 0;
      bring('tab:go', { index: spaceIndex, path: [0], at: to });
      const lowered = shell.mounted.tally.unmounted - before;
      must(lowered === 1, `switching one tab lowered ${lowered} faces; it must lower exactly the one it replaced`);
      must(shell.mounted.idAt('stage:1') === other, 'the other pane was remounted');
      return `one lowered here, none elsewhere (${other} held still)`;
    } finally {
      for (let i = 0; i < moved; i += 1) shell.undo();
    }
  });

  check('two hundred acts undo and redo to the same digest', () => {
    const random = seeded(WALK_SEED);
    const start = shell.digest;
    let done = 0;
    for (let i = 0; i < WALK_STEPS; i += 1) {
      const row = walkOnce(shell.act, shell.read, shell.state, random);
      if (row.outcome === 'moved') done += 1;
    }
    const end = shell.digest;
    for (let i = 0; i < done; i += 1) shell.undo();
    must(shell.digest === start, 'undoing everything did not come back to where it started');
    for (let i = 0; i < done; i += 1) shell.redo();
    must(shell.digest === end, 'redoing everything did not arrive where it left off');
    for (let i = 0; i < done; i += 1) shell.undo();
    return `${done} of 200 moved the shell, and both ends matched`;
  });

  check('the shell is where it opened', () => {
    must(shell.digest === opening, `the checks left the shell at ${shell.digest.slice(0, 20)} instead of ${opening.slice(0, 20)}`);
    return 'returned';
  });

  check('eval is not available to this document', () => {
    let threw = false;
    try { (0, eval)('1 + 1'); } catch { threw = true; }
    must(threw, 'eval ran, so the policy is not in force');
    return 'the policy refused eval';
  });

  const passed = results.filter((r) => r.ok).length;
  return { passed, total: results.length, results };
}

