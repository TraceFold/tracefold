// SPDX-License-Identifier: Apache-2.0
// The demo's one script. It hands the shell a manifest, a set of modules and a stand-in
// for the membrane, and then asks the window whether any of it is true.
//
// The result goes into the document title as well as onto the strip, because a window can
// be read from outside by its title and a screenshot can be read by a person. Two
// readings of the same number, one for a machine and one for an eye.

import { createShell } from '../kernel/shell.mjs';
import { formatMeasures } from '../kernel/measures.mjs';
import { MANIFEST } from './manifest.gen.mjs';
import { MODULES } from './modules.gen.mjs';
import { standInPort } from './port.mock.mjs';
import { run } from './checks.mjs';
// SS551's live status-bar numbers. W11 ("only the membrane reaches a network")
// bans fetch/XHR/WebSocket from every shipped file, so these are not fetched at
// runtime -- `tools/serve.mjs` reads `.run/report.json` (the whole app's suite
// report) and this package's own `.bench/report.json` off disk on every request
// and hands them back as an ordinary ES module, the same way the browser already
// reaches every other file here. A module that does not exist yet (this file's
// route was added by this lane) fails this import the same way any other
// missing file would; measures.mjs's formatMeasures() still reads null as
// "not wired" rather than throwing, so a stale dev server without the route is
// a degraded strip, not a broken page.
import { MEASURES } from '/.measures.gen.mjs';

const root = document.getElementById('shell-root');
const notices = [];

const shell = createShell({
  root,
  manifest: MANIFEST,
  modules: MODULES,
  port: standInPort(),
  notices,
});

const origin = typeof window !== 'undefined' && window.location ? window.location.origin : null;
shell.frame.showMeasures(formatMeasures({
  run: MEASURES?.run ?? null, bench: MEASURES?.bench ?? null, benchLabel: 'shell mount', origin,
  // req/822_c5 item 1: the same freshness basis the app window carries.
  now: MEASURES?.now ?? null,
}));

const outcome = run(shell, root);

// The checks leave the shell in light, which is where it opens. `?theme=dark` is here so
// that "dark is reachable" can be photographed rather than only measured -- the same act
// the keyboard and the rail use, asked for from the address bar.
const asked = new URL(window.location.href).searchParams.get('theme');
if (asked) shell.act('theme:set', { theme: asked });
const line = `${outcome.passed}/${outcome.total} checks`;
shell.frame.showChecks(line, outcome.passed === outcome.total);
document.title = `glovrex shell — ${line}`;
document.documentElement.dataset.checks = `${outcome.passed}/${outcome.total}`;
document.documentElement.dataset.checksOk = String(outcome.passed === outcome.total);

const report = document.getElementById('check-report');
for (const result of outcome.results) {
  const row = document.createElement('li');
  row.dataset.ok = String(result.ok);
  const name = document.createElement('span');
  name.className = 'check-name';
  name.textContent = result.name;
  const said = document.createElement('span');
  said.className = 'check-said';
  said.textContent = result.said;
  row.append(name, said);
  report.append(row);
}

// Kept reachable so the window can be questioned from a console without the shell having
// to publish anything to the page's global surface for its own sake.
window.gxShell = shell;
window.gxChecks = outcome;
