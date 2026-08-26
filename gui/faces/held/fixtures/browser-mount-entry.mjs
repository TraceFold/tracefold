// SPDX-License-Identifier: Apache-2.0
// The browser side of the W15 mount smoke for the held face. Runs in a real
// window, not in Node: this file is loaded as a native ES module by the page
// beside it, over http, and every import it makes is followed the same way a
// shipped shell's would be.

import { mount, face } from '../index.mjs';

// A minimal stand-in for the membrane's port: no network, one static candidate
// list. Written out rather than imported from a test file, for the same reason
// faces/ledger's own entry gives -- test tooling imports node:test, and pulling it
// into a browser page would be testing something this smoke does not claim to test.
function browserStubPort() {
  const held = [
    {
      id: 'c-101', sequence: 1, at: '2026-08-24T10:02:00Z', actor: 'agent:packer',
      effect: 'write', verdict: 'Escalate', path: '/work/contract.pdf', digest: '91aa47f0e6b2115d',
    },
    {
      id: 'c-102', sequence: 2, at: '2026-08-24T10:05:00Z', actor: 'agent:packer',
      effect: 'delete', verdict: 'Escalate', path: '/work/tmp/cache.bin', digest: '3c6b02d8ff41907e',
    },
  ];
  const walked = (items) => ({
    outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64,
  });
  return {
    async fold(name) {
      if (name === 'get_candidates') return walked(held);
      return { outcome: 'absent', reason: 'no_such_route', requested: { name } };
    },
  };
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
    dataRow: document.querySelectorAll('#host [data-row]').length,
  };
  window.__gxMountSmoke = census;
  document.title = `glovrex held face -- ${askedTheme === 'light' ? 'light' : 'dark'} -- elements ${census.elements} rows ${census.dataRow}`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
