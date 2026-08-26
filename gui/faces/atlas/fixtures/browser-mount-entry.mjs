// SPDX-License-Identifier: Apache-2.0
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
    text: (document.getElementById('host').innerText || '').replace(/\s+/g, '').length,
    dataSection: document.querySelectorAll('#host [data-section]').length,
    subjects: document.querySelectorAll('#host [data-role="subject"]').length,
    marksUnderFloor: [...document.querySelectorAll('#host svg[data-mark]')]
      .filter((s) => Math.round(s.getBoundingClientRect().width) < 16).length,
    menu: menuChecks,
  };
  window.__gxMountSmoke = census;
  document.title = `glovrex atlas face -- ${askedTheme === 'light' ? 'light' : 'dark'} -- elements ${census.elements} subjects ${census.subjects}`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
