// SPDX-License-Identifier: Apache-2.0
// The browser side of the W15 mount smoke. Runs in a real window, not in Node: this
// file itself is loaded as a native ES module by the page beside it, over http, and
// every import it makes is followed the same way a shipped shell's would be.

import { mount, face } from '../index.mjs';

// A minimal stand-in for the membrane's port: no network, one static ledger. Written
// out rather than imported from faces/ledger/test/stub-port.mjs, because that file
// is Node-test tooling (imports node:test) and pulling it into a browser page would
// be testing something this smoke does not claim to test.
function browserStubPort() {
  const settled = [
    {
      id: 't-001', sequence: 1, prev: null, at: '2026-08-24T09:01:00Z', actor: 'agent:packer',
      effect: 'write', verdict: 'Admit', path: '/work/report.md', digest: '4f10ab77c2d90013', basis: 'exact',
    },
    {
      id: 't-002', sequence: 2, prev: 't-001', at: '2026-08-24T09:04:00Z', actor: 'agent:packer',
      effect: 'delete', verdict: 'Deny', path: '/work/keys/private.pem', digest: 'b83e0c1547aa2260', basis: 'derived',
    },
  ];
  const held = [
    {
      id: 'c-101', sequence: 3, at: '2026-08-24T10:02:00Z', actor: 'agent:packer',
      effect: 'write', verdict: 'Escalate', path: '/work/contract.pdf', digest: '91aa47f0e6b2115d',
    },
  ];
  const walked = (items) => ({
    outcome: 'answered', items, requests: 1, pages: 1, stopped_at_budget: false, repeated_cursor: false, budget: 64,
  });
  const answers = {
    get_transformations: walked(settled),
    get_candidates: walked(held),
  };
  return {
    async fold(name) {
      const answer = answers[name];
      return answer ?? { outcome: 'absent', reason: 'no_such_route', requested: { name } };
    },
    async get_ledger_consistency() {
      return { outcome: 'answered', status: 200, body: { consistent: true, checked_from: 1, checked_to: 2 } };
    },
  };
}

// The page's own theme, not a renderer's emulated media: the stylesheet of record
// (tokens.css) declares dark on a bare :root and light only under
// :root[data-theme="light"], so a real window reaches light the same way a shipped
// shell's rail would -- by setting the attribute -- rather than through a devtools
// protocol call a headful window launched by real_window.ps1 has no route to.
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
  // The title carries the count back out to a process with no debugger port attached
  // to the window -- real_window.ps1 reads a window's title through the Win32 window
  // list, which is how the real-window capture proves what it photographed without a
  // CDP connection into a window it did not launch under a renderer.
  document.title = `glovrex ledger face -- ${askedTheme === 'light' ? 'light' : 'dark'} -- elements ${census.elements} rows ${census.dataRow}`;
}

run().catch((error) => {
  window.__gxMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
