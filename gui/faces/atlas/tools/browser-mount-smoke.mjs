// SPDX-License-Identifier: Apache-2.0
// The load-bearing evidence for req/02 W15's third clause, for this face: mounted
// into a real window rather than into the stand-in object test/dom-stand-in.mjs
// documents the reason for (imported from faces/ledger/test/, not duplicated).
//
// Nothing here reads a fixture written by toHtml(). The page this writes is the
// smallest possible host plus a native <script type="module"> that imports
// index.mjs -- the actual module a shell ships -- and calls mount() on it, against
// a stub port with no server behind it. If any file in that import graph still
// touched node:fs the module graph would fail to resolve in the browser and this
// would report SMOKE_MESSAGES.IMPORT_FAILED instead of a screenshot -- the
// negative control req/02 W15's AC column asks for.

import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { tokenSourceRealPath, sha256Hex } from '../../../parts/tools/token-source.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const ROOT = path.resolve(HERE, '../../..');
export const FIXTURE_DIR = path.resolve(HERE, '../fixtures');
const SHOT_DIR = path.join(FIXTURE_DIR, 'shots');

export const SMOKE_MESSAGES = {
  NO_RENDERER: 'no renderer was reached, so nothing here has been seen and no visual claim may be made',
  IMPORT_FAILED: 'the module graph did not resolve in the browser -- a node:* import is still reachable from it',
  EMPTY_MOUNT: 'the page loaded but the host stayed empty, which is not a pass (req/02 W15 clause 2)',
  MARK_UNDER_FLOOR: 'a mark reached a real window under the readable floor, whatever the source says it asked for',
  MENU_FELL: 'the right-click menu did not do what Owner #348 (2) asks of it, in a real window',
};

const ENTRY_NAME = 'browser-mount-entry.mjs';
const PAGE_NAME = 'browser-mount.html';

const ENTRY_SOURCE = `// SPDX-License-Identifier: Apache-2.0
// The browser side of the W15 mount smoke for the atlas face. Runs in a real
// window, not in Node: this file is loaded as a native ES module by the page
// beside it, over http, and every import it makes is followed the same way a
// shipped shell's would be.

import { mount, face } from '../index.mjs';

// A minimal stand-in for the membrane's port: no network, one folded list envelope
// naming three subjects, one of them (deck.pdf) touched twice so the fold-open
// path is exercised by the census below, not only the closed one. Written out
// rather than imported from a test file, for the same reason every other face's
// own entry gives -- test tooling imports node:test, and pulling it into a
// browser page would be testing something this smoke does not claim to test.
function browserStubPort() {
  const items = [
    { id: 't-301', sequence: 1, prev: null, at: '2026-08-24T11:01:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Admit', path: '/work/deck.pdf', digest: 'a1b2c3d4e5f60301' },
    { id: 't-302', sequence: 2, prev: 't-301', at: '2026-08-24T11:02:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Admit', path: '/work/deck.pdf', digest: 'a1b2c3d4e5f60302' },
    { id: 't-303', sequence: 3, prev: null, at: '2026-08-24T11:03:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Escalate', path: '/work/budget.xlsx', digest: 'a1b2c3d4e5f60303' },
  ];
  const envelope = { outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64 };
  return { async fold() { return envelope; } };
}

// The page's own theme, not a renderer's emulated media -- see faces/ledger's own
// entry for why: the stylesheet of record declares dark on a bare :root and light
// only under :root[data-theme="light"], and a headful window launched by
// real_window.ps1 has no route to a devtools protocol call.
const askedTheme = new URL(window.location.href).searchParams.get('theme');
if (askedTheme === 'light') document.documentElement.setAttribute('data-theme', 'light');

// Owner #348 (2), and SS24k: a static screenshot is not a verification of something
// that only exists once a hand has done something. These run against the real window
// -- real MouseEvents, the real top layer, the real clipboard -- because the three
// properties this atom asks to be pinned (Escape, click away, no stacking) are all
// properties of a document and none of them exist in the structural stand-in the unit
// tests mount into.
const menu = () => document.querySelector('#host [data-part="row-menu"]');
const rightClickOn = (node) => {
  const box = node.getBoundingClientRect();
  const at = { clientX: Math.round(box.left + 4), clientY: Math.round(box.top + 4), bubbles: true, cancelable: true };
  node.dispatchEvent(new PointerEvent('pointerdown', { ...at, button: 2 }));
  return node.dispatchEvent(new MouseEvent('contextmenu', { ...at, button: 2 }));
};
const key = (name) => document.dispatchEvent(new KeyboardEvent('keydown', { key: name, bubbles: true }));

async function menuPass() {
  const out = {};
  const cell = document.querySelector('#host [data-cell="at"][data-state="value"]');
  const line = document.querySelector('#host [data-role="subject-summary"]');
  out.cellFound = Boolean(cell);
  // 1. it opens, on the value under the pointer, and the page menu is refused
  out.defaultRefused = rightClickOn(cell) === false;
  out.opened = Boolean(menu());
  out.entries = menu() ? menu().querySelectorAll('[data-menu-entry]').length : 0;
  out.saysNoActs = menu() ? menu().textContent.includes(face.DECLARATION.acts_reason) : false;
  out.offersFullValue = menu()
    ? menu().querySelector('[data-menu-entry][data-enabled="true"]').getAttribute('data-menu-value') === cell.getAttribute('data-menu-value')
    : false;
  // 2. it is in the top layer, which is what stops a row's own overflow clipping it
  out.inTopLayer = menu() ? menu().matches(':popover-open') : false;
  const own = menu() ? menu().getBoundingClientRect() : null;
  out.placedAtPointer = own ? Math.round(own.left) > 0 && Math.round(own.top) > 0 : false;
  // 3. a second right-click does not stack a second one
  rightClickOn(line);
  out.menusAfterSecond = document.querySelectorAll('#host [data-part="row-menu"]').length;
  // 4. Escape closes it
  key('Escape');
  out.escapeClosed = menu() === null;
  // 5. a click away closes it
  rightClickOn(cell);
  out.reopened = Boolean(menu());
  document.body.dispatchEvent(new PointerEvent('pointerdown', { clientX: 2, clientY: 2, bubbles: true }));
  out.clickAwayClosed = menu() === null;
  // 6. copy says whether it worked rather than looking the same either way
  rightClickOn(cell);
  const take = menu().querySelector('[data-menu-entry][data-enabled="true"]');
  take.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
  await new Promise((resolve) => { setTimeout(resolve, 60); });
  const after = menu();
  out.copyStated = after ? after.hasAttribute('data-copied') || after.hasAttribute('data-copy-failed') : false;
  out.copied = after ? after.hasAttribute('data-copied') : false;
  out.copySaidOnScreen = after ? after.textContent.includes('copied') || after.textContent.includes('would not let') : false;
  out.clipboard = typeof navigator.clipboard;
  // 7. and the last one is left open on purpose, so the shot has a menu in it
  return out;
}

async function run() {
  const host = document.getElementById('host');
  const unmount = mount(host, browserStubPort(), []);
  await unmount.ready;
  const menuChecks = await menuPass();
  const census = {
    ready: true,
    faceId: face.DECLARATION.id,
    elements: document.querySelectorAll('#host *').length,
    text: (document.getElementById('host').innerText || '').replace(/\\s+/g, '').length,
    dataSection: document.querySelectorAll('#host [data-section]').length,
    subjects: document.querySelectorAll('#host [data-role="subject"]').length,
    marksUnderFloor: [...document.querySelectorAll('#host svg[data-mark]')]
      .filter((s) => Math.round(s.getBoundingClientRect().width) < 16).length,
    menu: menuChecks,
  };
  window.__gxMountSmoke = census;
  document.title = \`glovrex atlas face -- \${askedTheme === 'light' ? 'light' : 'dark'} -- elements \${census.elements} subjects \${census.subjects}\`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
`;

function pageHtml() {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>W15 real-browser mount smoke -- atlas face</title>
<link rel="stylesheet" href="/s-common/tokens.css">
<style>html,body{margin:0;padding:0;background:var(--bg);color:var(--ink);font:14px sans-serif}</style>
</head>
<body>
<div id="host"></div>
<script type="module" src="./${ENTRY_NAME}"></script>
</body>
</html>
`;
}

export function writeFixture() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  fs.writeFileSync(path.join(FIXTURE_DIR, ENTRY_NAME), ENTRY_SOURCE, 'utf8');
  fs.writeFileSync(path.join(FIXTURE_DIR, PAGE_NAME), pageHtml(), 'utf8');
  return { dir: FIXTURE_DIR, page: PAGE_NAME, entry: ENTRY_NAME };
}

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
};

/** A plain static file server over the project root, the same server every other
 * face's own smoke starts -- reused here as a fresh instance because the port and
 * its lifetime belong to this run, not shared across a process boundary. */
export function startStaticServer(root) {
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
        res.writeHead(200, { 'content-type': MIME['.css'], 'x-sha256': sha256Hex(bytes.toString('utf8')) });
        res.end(bytes);
        return;
      }
      const full = path.join(root, urlPath);
      if (!full.startsWith(root)) { res.writeHead(403); res.end(); return; }
      fs.readFile(full, (err, data) => {
        if (err) { res.writeHead(404); res.end(SMOKE_MESSAGES.IMPORT_FAILED); return; }
        res.writeHead(200, { 'content-type': MIME[path.extname(full)] ?? 'application/octet-stream' });
        res.end(data);
      });
    });
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

export async function shootMountSmoke() {
  const fixture = writeFixture();
  fs.mkdirSync(SHOT_DIR, { recursive: true });
  const server = await startStaticServer(ROOT);
  const { port } = server.address();
  const renderer = await startRenderer({ viewport: { width: 900, height: 900 } });
  let page;
  try {
    page = await renderer.openPage();
    const relative = path.relative(ROOT, path.join(fixture.dir, fixture.page)).split(path.sep).join('/');
    const url = `http://127.0.0.1:${port}/${relative}`;
    await page.open(url);
    await page.hold('window.__gxMountSmoke !== undefined');
    const census = await page.evaluate('window.__gxMountSmoke');
    const shot = path.join(SHOT_DIR, 'browser-mount-smoke.png');
    fs.writeFileSync(shot, await page.capture());
    const report = { url, shot, ...census };
    fs.writeFileSync(path.join(SHOT_DIR, 'browser-mount-smoke.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8');
    if (!census.ready) throw new Error(`${SMOKE_MESSAGES.IMPORT_FAILED}: ${census.error}`);
    if (census.elements === 0 || census.text === 0) throw new Error(SMOKE_MESSAGES.EMPTY_MOUNT);
    if (census.marksUnderFloor > 0) throw new Error(`${SMOKE_MESSAGES.MARK_UNDER_FLOOR}: ${census.marksUnderFloor}`);
    // Owner #348 (2). Every one of these is a property of a real document, and a
    // smoke that took the screenshot without asking would be the third time on this
    // project that a green run stood in for something nobody looked at. `copied` is
    // deliberately not on this list -- whether this window will hand a page the
    // clipboard is the window's decision, and the atom is that the menu says which.
    const asked = ['defaultRefused', 'opened', 'saysNoActs', 'offersFullValue', 'inTopLayer', 'escapeClosed', 'reopened', 'clickAwayClosed', 'copyStated', 'copySaidOnScreen'];
    const failed = asked.filter((name) => census.menu[name] !== true);
    if (census.menu.menusAfterSecond !== 1) failed.push(`menusAfterSecond=${census.menu.menusAfterSecond}`);
    if (failed.length > 0) throw new Error(`${SMOKE_MESSAGES.MENU_FELL}: ${failed.join(', ')}`);
    return report;
  } finally {
    if (page) await page.close().catch(() => {});
    await renderer.stop();
    server.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  shootMountSmoke().then((report) => {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  }).catch((error) => {
    process.stderr.write(`${SMOKE_MESSAGES.NO_RENDERER}: ${error.message}\n`);
    process.exitCode = 1;
  });
}
