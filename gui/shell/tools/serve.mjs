// SPDX-License-Identifier: Apache-2.0
// A bed for the window. Node's own http and nothing else.
//
// It exists for two reasons. The first is the policy: a page opened off the filesystem
// has an opaque origin, and `script-src 'self'` there means either everything or nothing
// depending on the browser, so a check that the policy holds would be checking the
// browser. Served from an origin, the policy is the policy.
//
// The second is colour. `/s-common/tokens.css` is answered from the one file that owns
// colour, wherever it actually lives, resolved and digested at start-up and printed. The
// bytes are never copied into this project: a second copy is a second palette on the day
// either one is edited, and both look right the day they are written.
//
// The third reason arrived with the membrane bind (req/803 gap 1). This page's policy
// says `connect-src 'self'`, and the engine answers on its own origin, so a window that
// asked the bed directly would be a window with a relaxed policy. Instead the bed carries
// the engine's base path through to it: the browser only ever talks to this origin, the
// policy stays exactly as strict as it was, and the bearer token stays on this side of
// the wire -- the window never holds it and cannot leak it. What the window is told is
// `/.bed.gen.mjs`: whether a bed was named at all, so a face can say "nothing was asked"
// and "the call went out and did not arrive" as the different facts they are.
//
//   node tools/serve.mjs [--port 8788] [--tokens <path>]
//                        [--bed http://127.0.0.1:8795] [--bed-token <hex> | --bed-token-file <path>]

import { createServer, request as httpRequest } from 'node:http';
import { readFileSync, realpathSync, existsSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { dirname, join, normalize } from 'node:path';
import { buildManifest } from '../../tools/rig/manifest.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = realpathSync(join(HERE, '..'));

const argOf = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? fallback : process.argv[at + 1];
};

// 2026-08-24 (req/03 SS635, glovrex/req/38): this used to point at
// TraceFold_App/ui_proto/ui/tokens.css, a live dependency on the retired reference
// tree (an SS24 retirement-methodology violation -- see tokens/tokens.css's own
// header). The stylesheet of record is now owned in-repo, one level above ROOT
// (shell/ -> glovrex_app -> tokens/tokens.css).
export const TOKENS_DEFAULT = join(ROOT, '..', 'tokens', 'tokens.css');
const tokensPath = argOf('--tokens', TOKENS_DEFAULT);
const port = Number(argOf('--port', '8788'));

/** The engine's own prefix (membrane/src/address.mjs BASE_PATH, gx-api/src/lib.rs:80). */
export const BED_PREFIX = '/v1/';
const bedOrigin = argOf('--bed', null);
const bedTokenFile = argOf('--bed-token-file', null);
const bedToken = argOf('--bed-token', bedTokenFile && existsSync(bedTokenFile)
  ? readFileSync(bedTokenFile, 'utf8').trim()
  : '');

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
};

const typeOf = (path) => TYPES[path.slice(path.lastIndexOf('.'))] ?? 'application/octet-stream';

export function tokenSource(path = tokensPath) {
  if (!existsSync(path)) return { path, found: false, sha: null, bytes: 0 };
  const real = realpathSync(path);
  const bytes = readFileSync(real);
  return { path: real, found: true, sha: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length };
}

/**
 * SS551's live status-bar numbers, generated fresh on every request rather than
 * fetched at runtime. W11 ("only the membrane reaches a network") bans `fetch`,
 * `XMLHttpRequest` and `new WebSocket` from every shipped browser file -- on
 * purpose, so every runtime network call is one the membrane's watched proxy
 * can see. Reading `.run/report.json` and this package's own `.bench/report.json`
 * is not a runtime call to a service at all, it is this dev bed reading two
 * files off the same disk it is already serving everything else from -- so it is
 * done here, server-side, and handed to the browser as an ordinary ES module a
 * plain `import` can load (which is how the browser already reaches every other
 * file in this application). `.run/report.json` lives one level above `ROOT`
 * (the whole app's report, not just this package's), which is why this route
 * reads it directly with node:fs rather than through the generic file handler
 * below -- that handler refuses anything outside `ROOT` on purpose (the same
 * boundary this route is a deliberate, narrow exception to, for exactly one
 * read-only file, never a write).
 */
/**
 * The tree digest, cached until the tree is SEEN to move.
 *
 * Three implementations were measured at this tree before this one was kept (the numbers
 * are the reason this comment exists -- the next person will be tempted by the same two):
 *
 *   full `buildManifest` per request      952ms, and it reads ~every byte of every
 *                                         capture round per window load; under
 *                                         `btn_verify`'s 56 sequential loads that IO
 *                                         storm reproducibly killed the renderer's
 *                                         devtools socket (crash at load ~32 of a
 *                                         bare-load probe, press-independent), and the
 *                                         one full GREEN run of the day was exactly the
 *                                         run that went through a cache.
 *   stat-signature walk per request       2132/2060/2396ms warm -- SLOWER than reading,
 *                                         on this OneDrive-backed disk. Built, timed,
 *                                         reverted.
 *   fs.watch dirty flag (this one)        ~0 steady-state; the full derivation runs only
 *                                         on the first ask and after a change event.
 *
 * Invalidation is by observed change, never by a clock. The stated caveat: a watcher
 * can miss an event (the OS API is best-effort), and the failure shape is a fresh claim
 * over an unobserved edit until the next event arrives. Events under `.run/` are ignored
 * because the digest itself excludes `.run` (tools/rig/manifest.mjs) -- a report write
 * must not invalidate a digest it cannot change.
 */
const treeCache = { dirty: true, tree: null };
try {
  const { watch } = await import('node:fs');
  const watcher = watch(join(ROOT, '..'), { recursive: true }, (event, filename) => {
    const name = String(filename ?? '').split('\\').join('/');
    if (name.startsWith('.run/') || name.startsWith('.git/') || name.startsWith('node_modules/')) return;
    treeCache.dirty = true;
  });
  // The watcher must never be the reason this module holds a process open: a test that
  // imports an export from this file is not a bed, and without this line it would hang.
  watcher.unref();
} catch { /* no watcher: every ask re-derives, which is the slow-and-honest fallback */ }

function measuresModule() {
  const readJson = (path) => {
    try { return JSON.parse(readFileSync(path, 'utf8')); } catch { return null; }
  };
  const reportAt = join(ROOT, '..', '.run', 'report.json');
  const run = readJson(reportAt);
  const bench = readJson(join(ROOT, '.bench', 'report.json'));
  // req/822_c5 item 1: what the report is ABOUT versus what this bed is SERVING. The
  // report carries `tree` (verify-all writes the digest of the manifest its assays were
  // handed); this computes the same digest of the tree as it stands right now, with the
  // same shipped derivation -- importing tools/rig/manifest.mjs rather than restating the
  // walk, because a copy of a digest algorithm is a second opinion of what the tree is
  // (the §234 rule). The derivation is full and shipped; only its INVALIDATION is cached
  // -- see treeCache above for the three measured implementations and why this one stands.
  // If the walk fails, `tree: null` -- the formatter then claims neither fresh nor stale.
  const now = { tree: null, atMs: Date.now(), reportMtimeMs: null };
  try {
    if (treeCache.dirty) {
      treeCache.tree = buildManifest(join(ROOT, '..')).treeDigest;
      treeCache.dirty = false;
    }
    now.tree = treeCache.tree;
  } catch { /* stated as null */ }
  try { now.reportMtimeMs = statSync(reportAt).mtimeMs; } catch { /* no report, no age */ }
  return `// SPDX-License-Identifier: Apache-2.0\n// Generated by tools/serve.mjs on every request. Do not edit: the disk is the source.\nexport const MEASURES = ${JSON.stringify({ run, bench, now })};\nexport default MEASURES;\n`;
}

/**
 * What this bed can say about the engine, stated rather than guessed.
 *
 * A window that cannot tell "no bed was named" from "a bed was named and did not answer"
 * will draw the same screen for both, and those are opposite facts -- the same distinction
 * `app/port.stand-in.mjs` was written to protect. So the window is handed the one bit it
 * cannot work out for itself, and derives nothing else: the token is not here, the origin
 * is not here (it talks to this bed, never to the engine), only whether to expect answers.
 */
function bedModule() {
  return `// SPDX-License-Identifier: Apache-2.0\n// Generated by tools/serve.mjs on every request. Do not edit: the command line is the source.\nexport const BED = ${JSON.stringify({ named: Boolean(bedOrigin), prefix: BED_PREFIX, tokenHeld: Boolean(bedToken) })};\nexport default BED;\n`;
}

/**
 * The engine's base path, carried through. Read-only in the sense that matters here: this
 * function decides nothing about the request except who to hand it to and which token to
 * attach. It does not read the body, does not classify the answer, and does not repair a
 * status -- classification is the membrane's job and doing it twice would give the window
 * two opinions of one call.
 */
function carryToBed(request, answer, asked) {
  const target = new URL(request.url, bedOrigin);
  const headers = { ...request.headers, host: target.host };
  // The window never holds the token, so it cannot send one. Attaching it here is what
  // makes `connect-src 'self'` and a real engine possible at the same time.
  if (bedToken) headers.authorization = `Bearer ${bedToken}`;
  const carried = httpRequest(
    { protocol: target.protocol, hostname: target.hostname, port: target.port, path: target.pathname + target.search, method: request.method, headers },
    (from) => {
      answer.writeHead(from.statusCode ?? 502, from.headers);
      from.pipe(answer);
    },
  );
  carried.on('error', (cause) => {
    // A bed that is not there is a transport failure and is answered as one, in the media
    // type the engine states refusals in -- so the membrane classifies it as `failed`
    // rather than mistaking this bed's own words for the engine's.
    answer.writeHead(502, { 'content-type': 'application/problem+json; charset=utf-8' });
    answer.end(JSON.stringify({
      type: 'about:blank', title: 'the bed named on the command line did not answer',
      status: 502, detail: `${bedOrigin}${asked}: ${cause?.message ?? cause}`,
    }));
  });
  request.pipe(carried);
}

const server = createServer((request, answer) => {
  const asked = decodeURIComponent(new URL(request.url, 'http://127.0.0.1').pathname);

  if (asked === '/.bed.gen.mjs') {
    answer.writeHead(200, { 'content-type': TYPES['.mjs'] });
    answer.end(bedModule());
    return;
  }

  if (asked.startsWith(BED_PREFIX)) {
    if (!bedOrigin) {
      answer.writeHead(503, { 'content-type': 'application/problem+json; charset=utf-8' });
      answer.end(JSON.stringify({
        type: 'about:blank', title: 'this bed was started without an engine to carry to',
        status: 503, detail: `pass --bed <origin> to reach one; ${asked} was asked of a bed that has none`,
      }));
      return;
    }
    carryToBed(request, answer, asked);
    return;
  }

  if (asked === '/.measures.gen.mjs') {
    answer.writeHead(200, { 'content-type': TYPES['.mjs'] });
    answer.end(measuresModule());
    return;
  }

  if (asked === '/s-common/tokens.css') {
    const source = tokenSource();
    if (!source.found) {
      answer.writeHead(503, { 'content-type': 'text/plain; charset=utf-8' });
      answer.end(`the single stylesheet that owns colour is not at ${tokensPath}; this shell does not carry a copy of it\n`);
      return;
    }
    answer.writeHead(200, { 'content-type': TYPES['.css'], 'x-source': source.path, 'x-sha256': source.sha });
    answer.end(readFileSync(source.path));
    return;
  }

  // req/97 gap-list item 2. `shell/app/` carries the six real faces, and a face lives
  // one level above this bed's own root (`faces/<id>/`) and draws with parts that live
  // beside it (`parts/src/`). Those two prefixes are named here, one at a time, and
  // resolved against the application root -- the same narrow, declared exception
  // `/s-common/tokens.css` above already is, and for the same reason: the alternative
  // is serving the whole disk, or keeping a second copy of every face under `shell/`,
  // and a second copy is a second face on the day either one is edited. Read-only,
  // never written, and still contained: a path that escapes its own prefix is refused
  // by the same check the general handler below applies.
  const APP_ROOT = realpathSync(join(ROOT, '..'));
  // `/membrane/` joins the two that were already here for the same reason they are: the
  // window imports the membrane's modules directly, and a second copy under `shell/` would
  // be a second membrane on the day either one is edited.
  const SHARED_PREFIXES = ['/faces/', '/parts/', '/membrane/'];
  const shared = SHARED_PREFIXES.find((prefix) => asked.startsWith(prefix));
  if (shared) {
    const at = normalize(join(APP_ROOT, asked));
    const under = normalize(join(APP_ROOT, shared));
    if (!at.startsWith(under)) {
      answer.writeHead(403, { 'content-type': 'text/plain; charset=utf-8' });
      answer.end('outside the shared tree\n');
      return;
    }
    if (!existsSync(at) || !statSync(at).isFile()) {
      answer.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
      answer.end(`no such file: ${asked}\n`);
      return;
    }
    answer.writeHead(200, { 'content-type': typeOf(at) });
    answer.end(readFileSync(at));
    return;
  }

  const path = normalize(join(ROOT, asked === '/' ? 'index.html' : asked));
  if (!path.startsWith(ROOT)) {
    answer.writeHead(403, { 'content-type': 'text/plain; charset=utf-8' });
    answer.end('outside the served tree\n');
    return;
  }
  if (!existsSync(path) || !statSync(path).isFile()) {
    answer.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
    answer.end(`no such file: ${asked}\n`);
    return;
  }
  answer.writeHead(200, { 'content-type': typeOf(path) });
  answer.end(readFileSync(path));
});

if (process.argv[1] && process.argv[1].endsWith('serve.mjs')) {
  const source = tokenSource();
  server.listen(port, '127.0.0.1', () => {
    process.stdout.write(`shell bed: http://127.0.0.1:${port}/\n`);
    process.stdout.write(source.found
      ? `tokens: ${source.path} sha256 ${source.sha.slice(0, 16)} (${source.bytes} bytes, not copied)\n`
      : `tokens: MISSING at ${tokensPath}\n`);
    process.stdout.write(bedOrigin
      ? `bed: ${BED_PREFIX}* carried to ${bedOrigin}${bedToken ? ' with a token this bed holds' : ' with NO token (the engine will refuse)'}\n`
      : `bed: none named, so ${BED_PREFIX}* answers 503 and every face will say it was not read\n`);
  });
}

export { server, port, ROOT };
