// SPDX-License-Identifier: Apache-2.0
// The browser side of the W15 mount smoke for the graph face. Runs in a real
// window, not in Node: this file is loaded as a native ES module by the page
// beside it, over http, and every import it makes is followed the same way a
// shipped shell's would be.

import { mount, face } from '../index.mjs';

// A minimal stand-in for the membrane's port: no network, one folded list envelope
// carrying a genuine in-window chain and an edge that leaves the window. Written
// out rather than imported from a test file, for the same reason every other
// face's own entry gives -- test tooling imports node:test, and pulling it into a
// browser page would be testing something this smoke does not claim to test.
function browserStubPort() {
  const items = [
    { id: 't-301', sequence: 1, prev: null, at: '2026-08-24T11:01:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Admit', path: '/work/deck.pdf', digest: 'e1a2b3c4d5f60301' },
    { id: 't-302', sequence: 2, prev: 't-301', at: '2026-08-24T11:02:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Admit', path: '/work/deck.pdf', digest: 'e1a2b3c4d5f60302' },
    { id: 't-303', sequence: 3, prev: 't-900', at: '2026-08-24T11:03:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Escalate', path: '/work/budget.xlsx', digest: 'e1a2b3c4d5f60303' },
    { id: 't-304', sequence: 4, prev: 't-303', at: '2026-08-24T11:04:00Z', actor: 'agent:packer', effect: 'write', verdict: 'Admit', path: '/work/budget.xlsx', digest: 'e1a2b3c4d5f60304' },
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

async function run() {
  const host = document.getElementById('host');
  const unmount = mount(host, browserStubPort(), []);
  await unmount.ready;
  const census = {
    ready: true,
    faceId: face.DECLARATION.id,
    elements: document.querySelectorAll('#host *').length,
    text: (document.getElementById('host').innerText || '').replace(/\s+/g, '').length,
    dataSection: document.querySelectorAll('#host [data-section]').length,
    pathGroups: document.querySelectorAll('#host [data-section="path-group"]').length,
    chainedRows: [...document.querySelectorAll('#host [data-child-of]')].filter((n) => n.getAttribute('data-child-of')).length,
    outsideAnnotations: document.querySelectorAll('#host [data-role="edge-outside"]').length,
  };
  window.__gxMountSmoke = census;
  document.title = `glovrex graph face -- ${askedTheme === 'light' ? 'light' : 'dark'} -- elements ${census.elements} groups ${census.pathGroups}`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
