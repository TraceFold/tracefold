// SPDX-License-Identifier: Apache-2.0
// W15's third clause read literally: a window a person could have looked at, not a
// headless dump of one. tools/browser-mount-smoke.mjs already proves the module
// graph resolves in a browser and that #host is not empty, using a real Chromium
// process that no person ever saw a window of. shell/tools/real_window.ps1 is the
// one instrument in this tree that opens Chrome in app mode, reads the window's own
// title back off the Win32 window list, and photographs the on-screen window
// rectangle -- this file points that instrument at the same fixture the mount smoke
// already writes, rather than building a second capture path for this face.
//
// Two runs, light and dark, because the token stylesheet declares dark on a bare
// :root and light only under :root[data-theme="light"]: the fixture's own entry
// script reads ?theme=light off its URL and sets the attribute itself, so this is
// two URLs against one server rather than two builds.
//
//   node tools/real-window-smoke.mjs

import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { writeMountFixture, startFileServer, APP_ROOT } from './browser-mount-smoke.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RECORD_DIR = path.resolve(HERE, '../record');
const REAL_WINDOW_SCRIPT = path.join(APP_ROOT, 'shell', 'tools', 'real_window.ps1');

export const REAL_WINDOW_MESSAGES = {
  POWERSHELL_DID_NOT_FINISH: 'powershell did not run to completion',
  NO_GLOVREX_WINDOW: 'no chrome window titled itself glovrex -- real_window.ps1 reported NO WINDOW',
  NO_SCREENSHOT_FILE: 'real_window.ps1 named a title but wrote no screenshot file',
  IMPLAUSIBLY_SMALL: 'the screenshot file is too small to plausibly hold a rendered window',
};

function parseRealWindowStdout(stdout) {
  return {
    title: /^TITLE: (.*)$/m.exec(stdout)?.[1] ?? null,
    rect: /^RECT: (.*)$/m.exec(stdout)?.[1] ?? null,
    shot: /^SHOT: (.*)$/m.exec(stdout)?.[1] ?? null,
    noWindow: /^NO WINDOW:/m.test(stdout),
  };
}

// spawn, not spawnSync: the static file server this smoke also starts runs in this
// same Node process, and a synchronous wait on the child would freeze the event loop
// that server needs in order to answer Chrome's own request for the page.
function runPowershellScript(args) {
  return new Promise((resolve) => {
    const child = spawn('powershell', args, { encoding: 'utf8' });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', (error) => resolve({ status: null, stdout, stderr, error }));
    child.on('close', (status) => resolve({ status, stdout, stderr, error: null }));
  });
}

async function captureTheme(theme, servedPort) {
  const url = theme === 'light'
    ? `http://127.0.0.1:${servedPort}/faces/notice/fixtures/mount.html?theme=light`
    : `http://127.0.0.1:${servedPort}/faces/notice/fixtures/mount.html`;
  const outName = `real_window_${theme}`;
  const run = await runPowershellScript([
    '-NoProfile', '-File', REAL_WINDOW_SCRIPT,
    '-Url', url, '-Out', RECORD_DIR, '-Name', outName, '-Wait', '6',
  ]);

  if (run.error || run.status !== 0) {
    throw new Error(`${REAL_WINDOW_MESSAGES.POWERSHELL_DID_NOT_FINISH}: ${run.error?.message ?? `exit ${run.status}`}\n${run.stderr ?? ''}`);
  }
  const parsed = parseRealWindowStdout(run.stdout ?? '');
  if (parsed.noWindow || !parsed.title) throw new Error(`${REAL_WINDOW_MESSAGES.NO_GLOVREX_WINDOW}\n${run.stdout}`);
  if (!parsed.shot || !fs.existsSync(parsed.shot)) throw new Error(REAL_WINDOW_MESSAGES.NO_SCREENSHOT_FILE);
  const byteSize = fs.statSync(parsed.shot).size;
  if (byteSize < 5000) throw new Error(`${REAL_WINDOW_MESSAGES.IMPLAUSIBLY_SMALL}: ${byteSize} bytes`);

  return { theme, url, ...parsed, bytes: byteSize };
}

export async function runRealWindowSmoke() {
  writeMountFixture();
  fs.mkdirSync(RECORD_DIR, { recursive: true });
  const server = await startFileServer(APP_ROOT);
  const { port } = server.address();
  try {
    const dark = await captureTheme('dark', port);
    const light = await captureTheme('light', port);
    // This script's job ends at the two screenshots plus the raw numbers below; it
    // does not overwrite record/real-window.json, which is the hand-authored record
    // (what/why/how/runs/readByEye) a person wrote after looking at the pictures --
    // matching faces/ledger's own division between "what the script measured" and
    // "what a person who looked at it is willing to say".
    const result = { face: 'notice', capturedAt: new Date().toISOString(), dark, light };
    return result;
  } finally {
    server.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runRealWindowSmoke().then((result) => {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  }).catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
