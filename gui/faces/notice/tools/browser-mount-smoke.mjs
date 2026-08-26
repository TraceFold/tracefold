// SPDX-License-Identifier: Apache-2.0
// req/02 W15's third clause, for this face: node --test proves this module against a
// stand-in host (test/dom-stand-in.mjs); it does not prove the module graph a real
// browser would load resolves, or that a real DOM ends up non-empty. This file is
// the evidence for that separate claim -- a page served over http, with a native
// `<script type="module">` importing index.mjs (the door a shell actually opens) and
// calling mount() on it, photographed by a headless renderer.
//
// C-7 makes this face's version of the smoke simpler than a route-reading face's:
// there is no port to stub, because this face is handed one and never calls it
// (notice.mjs `void port;`). What has to resolve in the browser is index.mjs,
// declaration.mjs, binding.mjs, and every part binding.mjs draws from -- if any of
// those still touched node:fs the import would fail in-page and this would report
// MOUNT_SMOKE_MESSAGES.MODULE_GRAPH_FAILED instead of a screenshot.

import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { tokenSourceRealPath, sha256Hex } from '../../../parts/tools/token-source.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const APP_ROOT = path.resolve(HERE, '../../..');
export const FIXTURE_DIR = path.resolve(HERE, '../fixtures');
const SHOTS_DIR = path.join(FIXTURE_DIR, 'shots');

export const MOUNT_SMOKE_MESSAGES = {
  RENDERER_UNREACHABLE: 'no renderer answered, so nothing here has been seen and no screenshot claim may be made',
  MODULE_GRAPH_FAILED: 'the module graph did not resolve inside the browser -- a node:* import is still reachable from index.mjs',
  HOST_STAYED_EMPTY: 'the page loaded but #host painted nothing (req/02 W15 clause 2)',
};

/** What the interaction pass says when a real event did not produce the screen it
 * should have. Each one names the property rather than the assertion. */
export const MENU_MESSAGES = {
  WRONG_COUNT: 'the document held the wrong number of menus after a real event',
  NOT_DRAWN: 'a right-click on a row drew no menu, or drew one with nothing in it',
  WRONG_ROW: 'the menu is about a row other than the one that was right-clicked',
  DID_NOT_MOVE: 'a second right-click on another row left the menu where it was',
  OFF_SCREEN: 'the menu hangs off an edge, so part of it cannot be pressed',
  SILENT_ABOUT_ACTS: 'the menu does not say that nothing on this screen can be sent',
  SILENT_COPY: 'a press on a copy control said nothing about whether it worked',
};

const ENTRY_FILE = 'mount-entry.mjs';
const PAGE_FILE = 'mount.html';

// Nine of the ten shapes declaration.mjs's ROWS names, plus one deliberately
// unrecognised outcome word and two non-record entries -- written here rather than
// imported from test/sample-notices.mjs, because that file exists to serve
// node --test and this smoke is proving a different thing (a real DOM, not a stand-in
// one) and should not inherit a dependency on test tooling to do it.
const ENTRY_SOURCE = `// SPDX-License-Identifier: Apache-2.0
// The browser side of the notice face's real-DOM mount smoke. Loaded as a native ES
// module over http by mount.html; every import below is followed exactly as a
// shipped shell's would be.

import { mount, face } from '../index.mjs';

function windowRecord() {
  return [
    { seq: 1, at: '2026-08-24T11:00:00.000Z', through: 'shell', method: 'get_transformations', outcome: 'asked' },
    {
      seq: 2, at: '2026-08-24T11:00:01.000Z', method: 'get_transformations', verb: 'GET', path: '/v1/transformations',
      outcome: 'answered', status: 200, result: { outcome: 'answered', status: 200, body: { items: [] } },
    },
    {
      seq: 3, at: '2026-08-24T11:00:02.000Z', method: 'post_candidates_id_commit', verb: 'POST', path: '/v1/candidates/{id}/commit',
      outcome: 'refused', status: 409,
      result: { outcome: 'refused', status: 409, gx_code: 'IDEMPOTENCY_CONFLICT', problem: { title: 'conflict', detail: 'this candidate was already committed' } },
    },
    {
      seq: 4, at: '2026-08-24T11:00:03.000Z', method: 'get_candidates', verb: 'GET', path: '/v1/candidates',
      outcome: 'failed', status: null, result: { outcome: 'failed', reason: 'transport', status: null, detail: 'the socket closed before an answer arrived' },
    },
    {
      seq: 5, at: '2026-08-24T11:00:04.000Z', method: 'get_everything_i_wish_for', verb: null, path: null,
      outcome: 'absent', status: null, result: { outcome: 'absent', reason: 'no_such_route', requested: { name: 'get_everything_i_wish_for' } },
    },
    { seq: 6, at: '2026-08-24T11:00:05.000Z', through: 'shell', method: 'pane:divide', outcome: 'refused', said: 'there is no act called "pane:divide" in this space' },
    { seq: 7, at: '2026-08-24T11:00:06.000Z', through: 'shell', method: 'theme:set', outcome: 'elsewhere', said: 'theme:set belongs to a different screen' },
    { seq: 8, at: '2026-08-24T11:00:07.000Z', method: 'get_transformations', verb: 'PATCH', path: '/v1/nowhere', outcome: 'partially_answered', status: 207 },
    'a string is not a record',
  ];
}

const requestedTheme = new URL(window.location.href).searchParams.get('theme');
if (requestedTheme === 'light') document.documentElement.setAttribute('data-theme', 'light');

async function run() {
  const host = document.getElementById('host');
  // C-7: this face never calls its port. An empty object stands in for one, and the
  // face's own \\\`void port;\\\` line is what makes that safe to hand it.
  const record = windowRecord();
  const unmount = mount(host, {}, record);
  // Owner #348 (2): the gutter control, in a real DOM. Where eight disabled buttons
  // stood there are now eight that act, and this counts them as elements rather than
  // as a claim about source text. It is also what the SS553 tap-floor scan
  // (tools/shoot.mjs) now sees: these are enabled, so they are inside its population.
  const offerButtons = [...document.querySelectorAll('#host [data-role="offer"]')];
  const census = {
    ready: true,
    faceId: face.DECLARATION.id,
    elements: document.querySelectorAll('#host *').length,
    textLength: (host.innerText || '').replace(/\\s+/g, '').length,
    dataSection: document.querySelectorAll('#host [data-section]').length,
    dataEntry: document.querySelectorAll('#host [data-entry]').length,
    offerControls: offerButtons.length,
    offerControlsLive: offerButtons.filter((b) => !b.disabled).length,
    offerControlsTitled: offerButtons.every((b) => (b.title || '').length > 20),
    reachControls: document.querySelectorAll('#host [data-role="reach"]').length,
    menusOpen: document.querySelectorAll('#host [data-role="menu"]').length,
  };
  window.__gxNoticeMountSmoke = census;
  // What the page under a real pointer needs to answer questions about itself. Every
  // one of these reads the live document rather than remembering anything.
  window.__gxNoticeProbe = {
    menus: () => document.querySelectorAll('[data-role="menu"]').length,
    menuBox: () => {
      const menu = document.querySelector('[data-role="menu"]');
      if (!menu) return null;
      const r = menu.getBoundingClientRect();
      return {
        entry: menu.getAttribute('data-menu-entry'),
        offers: menu.querySelectorAll('[data-role="offer"]').length,
        disabled: [...menu.querySelectorAll('[data-role="offer"]')].filter((b) => b.disabled).length,
        left: Math.round(r.left),
        top: Math.round(r.top),
        right: Math.round(r.right),
        bottom: Math.round(r.bottom),
        insideViewport: r.left >= 0 && r.top >= 0 && r.right <= window.innerWidth && r.bottom <= window.innerHeight,
        saysNoActs: /nothing on this screen can be sent/.test(menu.textContent || ''),
      };
    },
    rowRect: (nth) => {
      const row = document.querySelectorAll('#host [data-role="entry"]')[nth];
      if (!row) return null;
      const r = row.getBoundingClientRect();
      return { x: Math.round(r.left + 40), y: Math.round(r.top + r.height / 2), entry: row.getAttribute('data-menu-row') };
    },
    // A row the open menu is not sitting on top of. Picking one by index and hoping
    // is how this smoke first "proved" that a second right-click did not move the
    // menu: the coordinates of the row it picked were underneath the menu already
    // standing there, so the press landed on the menu. It did find a real defect
    // doing it -- the menu answered as its own row -- and the way not to depend on
    // that accident again is to choose a row that is actually visible.
    rowAwayFromMenu: () => {
      const menu = document.querySelector('[data-role="menu"]');
      const over = menu ? menu.getBoundingClientRect() : null;
      const subject = menu ? menu.getAttribute('data-menu-entry') : null;
      for (const row of document.querySelectorAll('#host [data-role="entry"]')) {
        const r = row.getBoundingClientRect();
        if (row.getAttribute('data-menu-row') === subject) continue;
        if (over && r.bottom > over.top && r.top < over.bottom && r.right > over.left && r.left < over.right) continue;
        return { x: Math.round(r.left + 40), y: Math.round(r.top + r.height / 2), entry: row.getAttribute('data-menu-row') };
      }
      return null;
    },
    menuPoint: () => {
      const menu = document.querySelector('[data-role="menu"]');
      if (!menu) return null;
      const r = menu.getBoundingClientRect();
      return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + 8) };
    },
    gutterRect: (nth) => {
      const button = document.querySelectorAll('#host [data-shape="gutter"]')[nth];
      if (!button) return null;
      const r = button.getBoundingClientRect();
      return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), w: Math.round(r.width), h: Math.round(r.height) };
    },
    copyState: () => {
      const took = document.querySelectorAll('[data-copied="true"]').length;
      const failed = document.querySelectorAll('[data-copy-failed="true"]').length;
      const marked = document.querySelector('[data-copied="true"],[data-copy-failed="true"]');
      return { took, failed, says: marked ? (marked.textContent || '').trim().slice(0, 60) : null };
    },
    // The window records another call while a menu is open -- the ordinary path on
    // this face, and the one req/103 finding 2 was found through. What this returns is
    // how many menus stand in the document after the whole screen was rebuilt.
    grow: () => {
      record.push({ seq: record.length + 1, at: '2026-08-24T11:00:09.000Z', through: 'shell', method: 'theme:set', outcome: 'asked' });
      unmount.repaint();
      return document.querySelectorAll('[data-role="menu"]').length;
    },
  };
  unmount.repaint();
  document.title = \`glovrex notice face -- \${requestedTheme === 'light' ? 'light' : 'dark'} -- elements \${census.elements} entries \${census.dataEntry}\`;
}

run().catch((error) => {
  window.__gxNoticeMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
`;

function pageMarkup() {
  // /s-common/tokens.css is the one route the shell's own static server answers for
  // the one stylesheet that owns colour (shell/tools/serve.mjs); this smoke's server
  // answers the same route the same way so a face's own smoke page and a real shell
  // agree on where var(--bg)/var(--ink) resolve from.
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>notice face real-DOM mount smoke</title>
<link rel="stylesheet" href="/s-common/tokens.css">
<style>html,body{margin:0;padding:0;background:var(--bg);color:var(--ink);font:14px sans-serif}</style>
</head>
<body>
<div id="host"></div>
<script type="module" src="./${ENTRY_FILE}"></script>
</body>
</html>
`;
}

export function writeMountFixture() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  fs.writeFileSync(path.join(FIXTURE_DIR, ENTRY_FILE), ENTRY_SOURCE, 'utf8');
  fs.writeFileSync(path.join(FIXTURE_DIR, PAGE_FILE), pageMarkup(), 'utf8');
  return { dir: FIXTURE_DIR, page: PAGE_FILE, entry: ENTRY_FILE };
}

const CONTENT_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
};

/** A static file server rooted at the repo, so relative ES-module imports resolve
 * the way they would under a shipped shell rather than as two opaque file: origins. */
export function startFileServer(root) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      const urlPath = decodeURIComponent(req.url.split('?')[0]);
      if (urlPath === '/s-common/tokens.css') {
        let bytes;
        try { bytes = fs.readFileSync(tokenSourceRealPath()); } catch (error) {
          res.writeHead(503, { 'content-type': 'text/plain; charset=utf-8' });
          res.end(`the single stylesheet that owns colour was not found: ${error.message}\n`);
          return;
        }
        res.writeHead(200, { 'content-type': CONTENT_TYPES['.css'], 'x-sha256': sha256Hex(bytes.toString('utf8')) });
        res.end(bytes);
        return;
      }
      const full = path.join(root, urlPath);
      if (!full.startsWith(root)) { res.writeHead(403); res.end(); return; }
      fs.readFile(full, (err, data) => {
        if (err) { res.writeHead(404); res.end(MOUNT_SMOKE_MESSAGES.MODULE_GRAPH_FAILED); return; }
        res.writeHead(200, { 'content-type': CONTENT_TYPES[path.extname(full)] ?? 'application/octet-stream' });
        res.end(data);
      });
    });
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

/**
 * A real right-click, a real Escape, a real press -- sent through the renderer's own
 * input pipeline rather than by building an event object in the page.
 *
 * SS24k: a still picture is not a check of an interaction. These three are what a
 * hand does to this face, and they are dispatched as input so that everything between
 * the device and the listener -- hit testing, the browser's own contextmenu
 * synthesis, focus -- is in the path being tested. A synthetic `dispatchEvent` would
 * skip all of it and still pass.
 */
function inputOn(page) {
  const mouse = async (type, x, y, button, clickCount) => page.raw.send('Input.dispatchMouseEvent', {
    type, x, y, button, clickCount, buttons: 0,
  });
  return {
    async rightClick(x, y) {
      await mouse('mousePressed', x, y, 'right', 1);
      await mouse('mouseReleased', x, y, 'right', 1);
      await page.settle();
    },
    async leftClick(x, y) {
      await mouse('mousePressed', x, y, 'left', 1);
      await mouse('mouseReleased', x, y, 'left', 1);
      await page.settle();
    },
    async escape() {
      await page.raw.send('Input.dispatchKeyEvent', {
        type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27,
      });
      await page.raw.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 });
      await page.settle();
    },
  };
}

/**
 * The interaction pass. Every line of it is a question asked of a live document after
 * a real event, and the answers go into the report beside the census -- so a run that
 * silently stopped interacting is a run with an empty pass, not a run that passed.
 */
async function interactionPass(page, shotsDir, fs_, path_) {
  const input = inputOn(page);
  const pass = { steps: [] };
  const step = async (name, expression) => {
    const value = await page.evaluate(expression);
    pass.steps.push({ name, value });
    return value;
  };

  const row = await page.evaluate('window.__gxNoticeProbe.rowRect(2)');
  if (!row) throw new Error('no row was drawn to right-click on');
  pass.row = row;

  await step('before any right-click', 'window.__gxNoticeProbe.menus()');
  await input.rightClick(row.x, row.y);
  await step('after a right-click', 'window.__gxNoticeProbe.menus()');
  pass.menu = await page.evaluate('window.__gxNoticeProbe.menuBox()');
  fs_.writeFileSync(path_.join(shotsDir, 'browser-mount-menu.png'), await page.capture());

  // A right-click on the open menu itself leaves it exactly where it is. It used to
  // reopen about the row the menu was drawn over, because the menu carried the same
  // attribute a row does and found itself.
  const onMenu = await page.evaluate('window.__gxNoticeProbe.menuPoint()');
  await input.rightClick(onMenu.x, onMenu.y);
  await step('after a right-click on the menu itself', 'window.__gxNoticeProbe.menus()');
  pass.onMenu = await page.evaluate('window.__gxNoticeProbe.menuBox()');

  // A second right-click, on a row the menu is not covering: one menu, and it moved.
  const other = await page.evaluate('window.__gxNoticeProbe.rowAwayFromMenu()');
  if (!other) throw new Error('no visible row outside the open menu to right-click on');
  pass.other = other;
  await input.rightClick(other.x, other.y);
  await step('after a second right-click on another row', 'window.__gxNoticeProbe.menus()');
  pass.second = await page.evaluate('window.__gxNoticeProbe.menuBox()');

  // The window records another call underneath the open menu.
  await step('after the window recorded another call', 'window.__gxNoticeProbe.grow()');

  await input.escape();
  await step('after Escape', 'window.__gxNoticeProbe.menus()');

  const again = await page.evaluate('window.__gxNoticeProbe.rowRect(2)');
  await input.rightClick(again.x, again.y);
  await step('reopened', 'window.__gxNoticeProbe.menus()');
  await input.leftClick(4, 4);
  await step('after a press away from it', 'window.__gxNoticeProbe.menus()');

  // And the one thing this face can genuinely do: a press on the gutter control.
  const gutter = await page.evaluate('window.__gxNoticeProbe.gutterRect(2)');
  pass.gutter = gutter;
  await input.leftClick(gutter.x, gutter.y);
  pass.copy = await page.evaluate('window.__gxNoticeProbe.copyState()');
  return pass;
}

export async function runMountSmoke() {
  const fixture = writeMountFixture();
  fs.mkdirSync(SHOTS_DIR, { recursive: true });
  const server = await startFileServer(APP_ROOT);
  const { port } = server.address();
  const renderer = await startRenderer({ viewport: { width: 900, height: 700 } });
  let page;
  try {
    page = await renderer.openPage();
    const relative = path.relative(APP_ROOT, path.join(fixture.dir, fixture.page)).split(path.sep).join('/');
    const url = `http://127.0.0.1:${port}/${relative}`;
    await page.open(url);
    await page.hold('window.__gxNoticeMountSmoke !== undefined');
    const census = await page.evaluate('window.__gxNoticeMountSmoke');
    const shotPath = path.join(SHOTS_DIR, 'browser-mount-smoke.png');
    fs.writeFileSync(shotPath, await page.capture());
    if (!census.ready) throw new Error(`${MOUNT_SMOKE_MESSAGES.MODULE_GRAPH_FAILED}: ${census.error}`);
    if (census.elements === 0 || census.textLength === 0) throw new Error(MOUNT_SMOKE_MESSAGES.HOST_STAYED_EMPTY);
    const pass = await interactionPass(page, SHOTS_DIR, fs, path);
    const report = {
      url, shot: shotPath, menuShot: path.join(SHOTS_DIR, 'browser-mount-menu.png'), ...census, interaction: pass,
    };
    fs.writeFileSync(path.join(SHOTS_DIR, 'browser-mount-smoke.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8');

    // Owner #348 (2), checked against a document a person could have been looking at.
    if (census.reachControls !== 0) throw new Error('the retired reach control is still drawn');
    if (census.offerControls === 0) throw new Error(`${MOUNT_SMOKE_MESSAGES.HOST_STAYED_EMPTY} (no row offered anything)`);
    if (census.offerControlsLive !== census.offerControls) throw new Error(`${census.offerControls - census.offerControlsLive} of ${census.offerControls} row controls drew dimmed`);
    if (!census.offerControlsTitled) throw new Error('a row control does not say what it would hand over');
    if (census.menusOpen !== 0) throw new Error('a menu was standing before anything asked for one');

    const answers = new Map(pass.steps.map((s) => [s.name, s.value]));
    const expect = (name, want) => {
      const got = answers.get(name);
      if (got !== want) throw new Error(`${MENU_MESSAGES.WRONG_COUNT}: ${name} -- wanted ${want}, the document had ${got}`);
    };
    expect('before any right-click', 0);
    expect('after a right-click', 1);
    expect('after a right-click on the menu itself', 1);
    expect('after a second right-click on another row', 1);
    if (pass.onMenu.entry !== pass.menu.entry || pass.onMenu.left !== pass.menu.left) throw new Error(`${MENU_MESSAGES.WRONG_ROW}: a right-click on the menu moved it or changed its subject`);
    if (pass.second.entry !== pass.other.entry) throw new Error(`${MENU_MESSAGES.WRONG_ROW}: ${pass.second.entry} for a right-click on ${pass.other.entry}`);
    expect('after the window recorded another call', 1);
    expect('after Escape', 0);
    expect('reopened', 1);
    expect('after a press away from it', 0);
    if (!pass.menu) throw new Error(MENU_MESSAGES.NOT_DRAWN);
    if (pass.menu.entry !== pass.row.entry) throw new Error(`${MENU_MESSAGES.WRONG_ROW}: ${pass.menu.entry} for a right-click on ${pass.row.entry}`);
    if (pass.second.entry === pass.menu.entry) throw new Error(MENU_MESSAGES.DID_NOT_MOVE);
    if (!pass.menu.insideViewport) throw new Error(`${MENU_MESSAGES.OFF_SCREEN}: ${JSON.stringify(pass.menu)}`);
    if (pass.menu.offers < 3) throw new Error(`${MENU_MESSAGES.NOT_DRAWN}: only ${pass.menu.offers} offers`);
    if (!pass.menu.saysNoActs) throw new Error(MENU_MESSAGES.SILENT_ABOUT_ACTS);
    // A press on the gutter control says which way the clipboard went. Either answer
    // is a pass -- a headless page without user activation is often refused, and
    // being refused and saying so is the behaviour being checked. Saying nothing is
    // the failure.
    if (pass.copy.took + pass.copy.failed !== 1) throw new Error(`${MENU_MESSAGES.SILENT_COPY}: ${JSON.stringify(pass.copy)}`);
    return report;
  } finally {
    if (page) await page.close().catch(() => {});
    await renderer.stop();
    server.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runMountSmoke().then((report) => {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  }).catch((error) => {
    process.stderr.write(`${MOUNT_SMOKE_MESSAGES.RENDERER_UNREACHABLE}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
