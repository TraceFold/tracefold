// SPDX-License-Identifier: Apache-2.0
// The load-bearing evidence for req/02 W15's third clause: this face, mounted into a
// real window rather than into the stand-in object test/dom-stand-in.mjs documents
// the reason for.
//
// Nothing here reads a fixture written by toHtml(). The page this writes is the
// smallest possible host plus a native <script type="module"> that imports
// index.mjs -- the actual module a shell ships -- and calls mount() on it, against a
// stub port with no server behind it. If any file in that import graph still touched
// node:fs (browser-mount-entry.mjs, ../index.mjs, ../binding.mjs, every part it
// draws with, tokens.mjs among them) the module graph would fail to resolve in the
// browser and this would report SMOKE_MESSAGES.IMPORT_FAILED instead of a
// screenshot -- the negative control req/02 W15's AC column asks for.

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
};

const ENTRY_NAME = 'browser-mount-entry.mjs';
const PAGE_NAME = 'browser-mount.html';

const ENTRY_SOURCE = `// SPDX-License-Identifier: Apache-2.0
// The browser side of the W15 mount smoke. Runs in a real window, not in Node: this
// file itself is loaded as a native ES module by the page beside it, over http, and
// every import it makes is followed the same way a shipped shell's would be.

import { mount, face } from '../index.mjs';

// A minimal stand-in for the membrane's port: no network, one static ledger. Written
// out rather than imported from faces/ledger/test/stub-port.mjs, because that file
// is Node-test tooling (imports node:test) and pulling it into a browser page would
// be testing something this smoke does not claim to test.
function browserStubPort() {
  const settled = [
    {
      id: 't-001', sequence: 1, prev: null, at: '2026-08-24T09:01:00Z', actor: 'agent:packer',
      effect: 'write', verdict: 'Admit', path: '/work/report.md', digest: '4f10ab77c2d90013', basis: 'exact',
    },
    {
      id: 't-002', sequence: 2, prev: 't-001', at: '2026-08-24T09:04:00Z', actor: 'agent:packer',
      effect: 'delete', verdict: 'Deny', path: '/work/keys/private.pem', digest: 'b83e0c1547aa2260', basis: 'derived',
    },
  ];
  const held = [
    {
      id: 'c-101', sequence: 3, at: '2026-08-24T10:02:00Z', actor: 'agent:packer',
      effect: 'write', verdict: 'Escalate', path: '/work/contract.pdf', digest: '91aa47f0e6b2115d',
    },
  ];
  const walked = (items) => ({
    outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64,
  });
  const answers = {
    get_transformations: walked(settled),
    get_candidates: walked(held),
  };
  return {
    async fold(name) {
      const answer = answers[name];
      return answer ?? { outcome: 'absent', reason: 'no_such_route', requested: { name } };
    },
    async get_ledger_consistency() {
      return { outcome: 'answered', status: 200, body: { consistent: true, checked_from: 1, checked_to: 2 } };
    },
  };
}

// The page's own theme, not a renderer's emulated media: the stylesheet of record
// (tokens.css) declares dark on a bare :root and light only under
// :root[data-theme="light"], so a real window reaches light the same way a shipped
// shell's rail would -- by setting the attribute -- rather than through a devtools
// protocol call a headful window launched by real_window.ps1 has no route to.
const askedTheme = new URL(window.location.href).searchParams.get('theme');
if (askedTheme === 'light') document.documentElement.setAttribute('data-theme', 'light');

async function run() {
  const host = document.getElementById('host');
  const unmount = mount(host, browserStubPort(), []);
  await unmount.ready;
  const census = {
    ready: true,
    faceId: face.DECLARATION.id,
    elements: document.querySelectorAll('#host *').length,
    text: (document.getElementById('host').innerText || '').replace(/\\s+/g, '').length,
    dataSection: document.querySelectorAll('#host [data-section]').length,
    dataRow: document.querySelectorAll('#host [data-row]').length,
  };
  window.__gxMountSmoke = census;
  // The title carries the count back out to a process with no debugger port attached
  // to the window -- real_window.ps1 reads a window's title through the Win32 window
  // list, which is how the real-window capture proves what it photographed without a
  // CDP connection into a window it did not launch under a renderer.
  document.title = \`glovrex ledger face -- \${askedTheme === 'light' ? 'light' : 'dark'} -- elements \${census.elements} rows \${census.dataRow}\`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
`;

function pageHtml() {
  // /s-common/tokens.css names the same served-off-the-canonical-file route
  // shell/tools/serve.mjs answers (shell/index.html carries the identical link and
  // comment) -- one route name for the one stylesheet, so a face's own smoke page and
  // the shell that will actually mount it agree on where colour comes from. Without
  // this link every var(--bg)/var(--ink)/var(--row) the face's inline styles read
  // resolves to nothing and the page renders in whatever default the browser applies,
  // which is a mount proof but not a token-fidelity one (req/02 W15 clause 2's styled
  // screenshot needs the second thing).
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>W15 real-browser mount smoke -- ledger face</title>
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

/**
 * A plain static file server over the project root, used only so the page below can
 * be reached at an http: URL. A native ES module's relative imports do not reliably
 * resolve across two documents both opened as file: in this renderer -- each file:
 * document is its own opaque origin -- so this smoke is served the way a shipped
 * shell would be served, rather than opened directly off disk the way the toHtml()
 * fixtures are.
 */
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
  const renderer = await startRenderer({ viewport: { width: 900, height: 700 } });
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
