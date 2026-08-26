// SPDX-License-Identifier: Apache-2.0
// The app window's one script: the same shell, carrying the six real faces.
//
// req/97's gap-list item 2 found that `shell/demo/modules.gen.mjs` imported seven demo
// placeholders and none of the six faces this repository actually builds -- so the
// machinery for req/768 AC-1 (a rail, a launcher column, a tab strip) existed with
// nothing real wired into it, and there was no surface anywhere in the tree from which
// a viewer could reach a second real face. This page is that surface.
//
// Nothing here names a face. `shell/app/manifest.gen.mjs` and `shell/app/modules.gen.mjs`
// are written by the same `tools/gen_manifest.mjs` the demo pair is written by, pointed
// at `faces/` instead of `shell/demo/faces/`: the folder is the source, so deleting a
// face's folder removes it from this window without a line here changing.

import { createShell } from '../kernel/shell.mjs';
import { formatMeasures } from '../kernel/measures.mjs';
import { MANIFEST } from './manifest.gen.mjs';
import { MODULES } from './modules.gen.mjs';
import { windowPort } from './port.membrane.mjs';
import { MEASURES } from '/.measures.gen.mjs';
import { BED } from '/.bed.gen.mjs';

const root = document.getElementById('shell-root');
const notices = [];

// req/803 gap 1. Which port this is, is not decided here and is not guessed: the bed says
// whether an engine was named for it, and `windowPort` binds the membrane or keeps the
// stand-in and states which. Both states are drawn; neither is inferred from a screen.
const origin = typeof window !== 'undefined' && window.location ? window.location.origin : null;
const wired = windowPort({ bed: BED, origin, notices });

const shell = createShell({
  root,
  manifest: MANIFEST,
  modules: MODULES,
  port: wired.port,
  notices,
});

shell.frame.showMeasures(formatMeasures({
  run: MEASURES?.run ?? null, bench: MEASURES?.bench ?? null, benchLabel: 'shell mount', origin,
  // req/822_c5 item 1: the bed's own measurement of the tree it serves, so the strip can
  // say when the report it draws is about another tree instead of presenting it as now.
  now: MEASURES?.now ?? null,
}));

/**
 * What this window can be asked, in the window. A face can be declared, imported,
 * syntactically perfect and still draw nothing (shell/record/real-window.json's own
 * "why"), so the count below is computed from the live document after mounting and
 * written into the title, where a capture with no debugger attached can read it.
 */
function census() {
  const hosts = [...root.querySelectorAll('[data-host]')];
  const stood = hosts.map((host) => ({
    id: host.getAttribute('data-host'),
    elements: host.querySelectorAll('*').length,
    characters: (host.innerText || '').replace(/\s+/g, '').length,
    faceRoot: host.querySelector('[data-face]')?.getAttribute('data-face') ?? null,
  }));
  const declared = MANIFEST.faces.map((f) => f.id);
  const drew = stood.filter((s) => s.faceRoot !== null && s.characters > 0);
  return { declared, stood, drew: drew.length, placed: stood.length };
}

const asked = new URL(window.location.href).searchParams.get('theme');
if (asked) shell.act('theme:set', { theme: asked });

const seen = census();
// Both numbers, never the first alone: the stage is a tab strip, so a face that is
// declared and reachable is not necessarily mounted right now, and a line reading
// "3/3" with the six unstated would be the denominator defect this whole tree is
// built against.
const line = `${seen.drew}/${seen.placed} mounted drew, ${seen.declared.length} real faces declared`;
// req/822_c7 S5 (Owner #388): the strip draws the figure; the sentence rides its
// title (and the document title below, which is not screen room). Both numbers are
// still both numbers -- the compact form keeps the denominator.
shell.frame.showChecks(`faces ${seen.drew}/${seen.placed}`, seen.drew === seen.placed && seen.placed > 0, line);
document.title = `glovrex app -- ${line}`;
document.documentElement.dataset.realFaces = `${seen.drew}/${seen.placed}`;
document.documentElement.dataset.declared = String(seen.declared.length);

const report = document.getElementById('check-report');
if (report) {
  for (const stood of seen.stood) {
    const row = document.createElement('li');
    row.dataset.ok = String(stood.faceRoot !== null && stood.characters > 0);
    const name = document.createElement('span');
    name.className = 'check-name';
    name.textContent = stood.id;
    const said = document.createElement('span');
    said.className = 'check-said';
    said.textContent = `${stood.elements} elements, ${stood.characters} characters, face root ${stood.faceRoot ?? 'none'}`;
    row.append(name, said);
    report.append(row);
  }
  report.hidden = false;
}

// Said on the page, not only in the module that produces it -- and now it is one of two
// sentences rather than one, because this window has two states it can honestly be in.
const banner = document.getElementById('stand-in-said');
if (banner) {
  banner.textContent = wired.said;
  banner.dataset.bound = String(wired.bound);
}
document.documentElement.dataset.bound = String(wired.bound);

window.gxShell = shell;
window.gxFaces = seen;
// The port itself, so a check can ask the wire a question instead of reading the screen
// for reassurance -- a face may decide not to draw a field, and that is a drawing
// decision and not evidence about what came back (`shell/tools/bound_smoke.mjs` B4).
window.gxWired = { bound: wired.bound, said: wired.said, notices, port: wired.port };
