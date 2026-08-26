// SPDX-License-Identifier: Apache-2.0
// The browser side of the W15 mount smoke for the receipt face. Runs in a real
// window, not in Node: this file is loaded as a native ES module by the page
// beside it, over http, and every import it makes is followed the same way a
// shipped shell's would be.

import { mount, face } from '../index.mjs';

// A minimal stand-in for the membrane's port: no network, one static delta and its
// receipt. Written out rather than imported from a test file, for the same reason
// every other face's own entry gives -- test tooling imports node:test, and
// pulling it into a browser page would be testing something this smoke does not
// claim to test.
function browserStubPort() {
  const digest = '91aa47f0e6b2115d';
  const delta = {
    outcome: 'answered', status: 200,
    body: {
      id: 't-201', sequence: 1, prev: null, at: '2026-08-24T10:02:00Z', actor: 'agent:packer',
      effect: 'write', verdict: 'Admit', path: '/work/contract.pdf', digest,
    },
  };
  const receipt = {
    outcome: 'answered', status: 200,
    body: { digest, algorithm: 'sha256', anchor: 'https://example.test/anchor/t-201', basis: 'exact' },
  };
  return {
    async get_transformations_id() { return delta; },
    async get_receipts_tid() { return receipt; },
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
  host.setAttribute('data-receipt-id', 't-201');
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
  document.title = `glovrex receipt face -- ${askedTheme === 'light' ? 'light' : 'dark'} -- elements ${census.elements} sections ${census.dataSection}`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
