// SPDX-License-Identifier: Apache-2.0
// The evidence req/02 W15's third clause actually asks for: this face on a screen a
// person could have looked at. Reuses shell/tools/real_window.ps1 -- the one
// instrument in this tree that opens Chrome in app mode, reads the window's own
// title back off the Win32 window list, and photographs the window rectangle --
// against this face's own fixture, instead of building a second capture path.
//
//   node tools/real-window-smoke.mjs

import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { writeFixture, startStaticServer, ROOT } from './browser-mount-smoke.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RECORD_DIR = path.resolve(HERE, '../record');
const REAL_WINDOW_PS1 = path.join(ROOT, 'shell', 'tools', 'real_window.ps1');

export const REAL_WINDOW_MESSAGES = {
  NO_POWERSHELL: 'powershell did not run to completion',
  NO_WINDOW: 'no chrome window called itself glovrex -- real_window.ps1 reported NO WINDOW',
  NO_SHOT: 'real_window.ps1 produced a title but no screenshot file',
  BLANK_SHOT: 'the screenshot file is implausibly small to hold a rendered window',
};

function parsePowershellOutput(stdout) {
  const title = /^TITLE: (.*)$/m.exec(stdout)?.[1] ?? null;
  const rect = /^RECT: (.*)$/m.exec(stdout)?.[1] ?? null;
  const shot = /^SHOT: (.*)$/m.exec(stdout)?.[1] ?? null;
  const noWindow = /^NO WINDOW:/m.test(stdout);
  return { title, rect, shot, noWindow };
}

// child_process.spawn, not spawnSync -- the static server this file also starts
// lives in this same Node process, and req/03 §4-6 already found the failure mode
// spawnSync produces here (the server frozen for the whole wait, Chrome loading the
// bare URL as its title). Async spawn keeps the server answering while PowerShell
// runs.
function runPowershell(args) {
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

async function captureOne(theme, port) {
  const url = theme === 'light'
    ? `http://127.0.0.1:${port}/faces/receipt/fixtures/browser-mount.html?theme=light`
    : `http://127.0.0.1:${port}/faces/receipt/fixtures/browser-mount.html`;
  const name = `real_window_${theme}`;
  const run = await runPowershell([
    '-NoProfile', '-File', REAL_WINDOW_PS1,
    '-Url', url, '-Out', RECORD_DIR, '-Name', name, '-Wait', '6',
  ]);

  if (run.error || run.status !== 0) {
    throw new Error(`${REAL_WINDOW_MESSAGES.NO_POWERSHELL}: ${run.error?.message ?? `exit ${run.status}`}\n${run.stderr ?? ''}`);
  }
  const parsed = parsePowershellOutput(run.stdout ?? '');
  if (parsed.noWindow || !parsed.title) throw new Error(`${REAL_WINDOW_MESSAGES.NO_WINDOW}\n${run.stdout}`);
  if (!parsed.shot || !fs.existsSync(parsed.shot)) throw new Error(REAL_WINDOW_MESSAGES.NO_SHOT);
  const bytes = fs.statSync(parsed.shot).size;
  if (bytes < 5000) throw new Error(`${REAL_WINDOW_MESSAGES.BLANK_SHOT}: ${bytes} bytes`);

  return { theme, url, ...parsed, bytes };
}

export async function shootRealWindow() {
  writeFixture();
  fs.mkdirSync(RECORD_DIR, { recursive: true });
  const server = await startStaticServer(ROOT);
  const { port } = server.address();
  try {
    const dark = await captureOne('dark', port);
    const light = await captureOne('light', port);
    return { dark, light };
  } finally {
    server.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  shootRealWindow().then((result) => {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  }).catch((error) => {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  });
}
