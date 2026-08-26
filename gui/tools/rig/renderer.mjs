// SPDX-License-Identifier: Apache-2.0
// Standing a renderer up, pinning everything about it that would otherwise move,
// and handing back a page that only ever waits on a predicate.
//
// req/06 §3-3 bans a wait measured in milliseconds. Every wait here is an
// in-page promise that settles when the condition holds, with the renderer's own
// timeout as the upper bound -- so "could not wait" arrives as a failure and never
// as a quiet pass.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { openCdp } from './cdp.mjs';

export const RENDERER_MESSAGES = {
  NOT_FOUND: 'no renderer binary at any known location',
  NO_ENDPOINT: 'the renderer started but never announced a debugging endpoint',
  PREDICATE_TIMEOUT: 'the predicate did not hold before the upper bound',
  STARTUP_CEILING: 'the renderer never announced a debugging endpoint and never exited',
  READY: 'renderer ready',
};

// The ceiling on standing a renderer up. A renderer that neither speaks nor dies is
// the one failure a predicate cannot answer on its own, and it was measured here:
// a stalled process held a whole run open with nothing written and nothing wrong.
const STARTUP_CEILING_MS = 30000;

const CANDIDATE_BINARIES = [
  'C:/Program Files/Google/Chrome/Application/chrome.exe',
  'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe',
  `${process.env.LOCALAPPDATA ?? ''}/Google/Chrome/Application/chrome.exe`,
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
];

export function findRenderer() {
  for (const candidate of CANDIDATE_BINARIES) {
    if (candidate && fs.existsSync(candidate)) return candidate;
  }
  return null;
}

// Everything on this list either removes a source of drift between two runs of the
// same page or removes a source of drift between two machines. The env digest
// records the ones that cannot be removed (font set, renderer build).
const PINNING_FLAGS = [
  '--headless=new',
  '--disable-gpu',
  '--hide-scrollbars',
  '--force-device-scale-factor=1',
  '--force-color-profile=srgb',
  '--disable-lcd-text',
  '--disable-font-subpixel-positioning',
  '--font-render-hinting=none',
  '--disable-partial-raster',
  '--run-all-compositor-stages-before-draw',
  '--disable-background-timer-throttling',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--disable-ipc-flooding-protection',
  '--disable-extensions',
  '--disable-sync',
  '--no-first-run',
  '--no-default-browser-check',
  '--mute-audio',
  '--remote-debugging-port=0',
];

export async function startRenderer({ binary = findRenderer(), viewport = { width: 1280, height: 800 } } = {}) {
  if (!binary) throw new Error(RENDERER_MESSAGES.NOT_FOUND);
  const profile = path.join(os.tmpdir(), `gxapp-rig-${crypto.randomUUID()}`);
  fs.mkdirSync(profile, { recursive: true });

  const child = spawn(binary, [...PINNING_FLAGS, `--user-data-dir=${profile}`, 'about:blank'], { stdio: ['ignore', 'pipe', 'pipe'] });

  // A predicate with no ceiling is a hang, not a wait. This one is bounded, and
  // reaching the bound is a failure with a sentence -- never a quiet skip. The bound
  // is an AbortSignal rather than a timer because a timer that resolves is how a
  // duration sneaks back in as the success path; this one can only reject.
  const endpoint = await new Promise((resolve, reject) => {
    let stderr = '';
    const ceiling = AbortSignal.timeout(STARTUP_CEILING_MS);
    const onData = (chunk) => {
      stderr += chunk.toString();
      const m = /DevTools listening on (ws:\/\/\S+)/.exec(stderr);
      if (m) { child.stderr.off('data', onData); resolve(m[1]); }
    };
    child.stderr.on('data', onData);
    child.on('exit', (code) => reject(new Error(`${RENDERER_MESSAGES.NO_ENDPOINT} (exit ${code})\n${stderr.slice(0, 400)}`)));
    ceiling.addEventListener('abort', () => {
      child.kill();
      reject(new Error(`${RENDERER_MESSAGES.STARTUP_CEILING} after ${STARTUP_CEILING_MS}ms\n${stderr.slice(0, 400)}`));
    });
  });

  const cdp = await openCdp(endpoint);
  const version = await cdp.call('Browser.getVersion');

  return {
    binary,
    endpoint,
    product: version.product,
    userAgent: version.userAgent,

    async openPage() {
      const page = await cdp.newPage();
      await page.send('Emulation.setDeviceMetricsOverride', {
        width: viewport.width, height: viewport.height, deviceScaleFactor: 1, mobile: false,
      });
      // A page whose animations still run is a page that renders differently at two
      // instants; freezing the clock is what makes a second capture comparable.
      await page.send('Emulation.setCPUThrottlingRate', { rate: 1 });
      try { await page.send('Animation.setPlaybackRate', { playbackRate: 0 }); } catch { /* domain absent */ }
      return wrapPage(page, viewport);
    },

    async stop() {
      try { cdp.close(); } catch { /* already closed */ }
      child.kill();
      try { fs.rmSync(profile, { recursive: true, force: true }); } catch { /* the OS will */ }
    },
  };
}

function wrapPage(page, viewport) {
  return {
    raw: page,
    viewport,

    async open(fileUrl) {
      await page.send('Page.navigate', { url: fileUrl });
      await this.settle();
    },

    // The one wait in this file. It is a condition, not a duration.
    async hold(expression, { boundMs = 10000 } = {}) {
      const wrapped = `new Promise((resolve, reject) => {
        const holds = () => { try { return Boolean(${expression}); } catch (e) { return false; } };
        const tick = () => { if (holds()) resolve(true); else requestAnimationFrame(tick); };
        tick();
      })`;
      const result = await page.send('Runtime.evaluate', { expression: wrapped, awaitPromise: true, timeout: boundMs, returnByValue: true });
      if (result.exceptionDetails) throw new Error(`${RENDERER_MESSAGES.PREDICATE_TIMEOUT}: ${expression}`);
      return true;
    },

    // Loaded, fonts resolved, and two frames presented -- the last one because a
    // capture taken in the same frame as layout catches a half-painted tree.
    async settle() {
      await this.hold("document.readyState === 'complete'");
      await this.hold("document.fonts.status === 'loaded'");
      await page.send('Runtime.evaluate', {
        expression: 'new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))',
        awaitPromise: true, timeout: 10000,
      });
    },

    async evaluate(expression) {
      const result = await page.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
      if (result.exceptionDetails) {
        const d = result.exceptionDetails;
        throw new Error(`evaluation threw: ${d.exception?.description ?? d.text}`);
      }
      return result.result.value;
    },

    async capture() {
      const { data } = await page.send('Page.captureScreenshot', {
        format: 'png',
        captureBeyondViewport: false,
        fromSurface: true,
        clip: { x: 0, y: 0, width: viewport.width, height: viewport.height, scale: 1 },
      });
      return Buffer.from(data, 'base64');
    },

    close: () => page.close(),
  };
}
