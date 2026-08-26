// SPDX-License-Identifier: Apache-2.0
// The browser side of the notice face's real-DOM mount smoke. Loaded as a native ES
// module over http by mount.html; every import below is followed exactly as a
// shipped shell's would be.

import { mount, face } from '../index.mjs';

function windowRecord() {
  return [
    { seq: 1, at: '2026-08-24T11:00:00.000Z', through: 'shell', method: 'get_transformations', outcome: 'asked' },
    {
      seq: 2, at: '2026-08-24T11:00:01.000Z', method: 'get_transformations', verb: 'GET', path: '/v1/transformations',
      outcome: 'answered', status: 200, result: { outcome: 'answered', status: 200, body: { items: [] } },
    },
    {
      seq: 3, at: '2026-08-24T11:00:02.000Z', method: 'post_candidates_id_commit', verb: 'POST', path: '/v1/candidates/{id}/commit',
      outcome: 'refused', status: 409,
      result: { outcome: 'refused', status: 409, gx_code: 'IDEMPOTENCY_CONFLICT', problem: { title: 'conflict', detail: 'this candidate was already committed' } },
    },
    {
      seq: 4, at: '2026-08-24T11:00:03.000Z', method: 'get_candidates', verb: 'GET', path: '/v1/candidates',
      outcome: 'failed', status: null, result: { outcome: 'failed', reason: 'transport', status: null, detail: 'the socket closed before an answer arrived' },
    },
    {
      seq: 5, at: '2026-08-24T11:00:04.000Z', method: 'get_everything_i_wish_for', verb: null, path: null,
      outcome: 'absent', status: null, result: { outcome: 'absent', reason: 'no_such_route', requested: { name: 'get_everything_i_wish_for' } },
    },
    { seq: 6, at: '2026-08-24T11:00:05.000Z', through: 'shell', method: 'pane:divide', outcome: 'refused', said: 'there is no act called "pane:divide" in this space' },
    { seq: 7, at: '2026-08-24T11:00:06.000Z', through: 'shell', method: 'theme:set', outcome: 'elsewhere', said: 'theme:set belongs to a different screen' },
    { seq: 8, at: '2026-08-24T11:00:07.000Z', method: 'get_transformations', verb: 'PATCH', path: '/v1/nowhere', outcome: 'partially_answered', status: 207 },
    'a string is not a record',
  ];
}

const requestedTheme = new URL(window.location.href).searchParams.get('theme');
if (requestedTheme === 'light') document.documentElement.setAttribute('data-theme', 'light');

async function run() {
  const host = document.getElementById('host');
  // C-7: this face never calls its port. An empty object stands in for one, and the
  // face's own \`void port;\` line is what makes that safe to hand it.
  const record = windowRecord();
  const unmount = mount(host, {}, record);
  // Owner #348 (2): the gutter control, in a real DOM. Where eight disabled buttons
  // stood there are now eight that act, and this counts them as elements rather than
  // as a claim about source text. It is also what the SS553 tap-floor scan
  // (tools/shoot.mjs) now sees: these are enabled, so they are inside its population.
  const offerButtons = [...document.querySelectorAll('#host [data-role="offer"]')];
  const census = {
    ready: true,
    faceId: face.DECLARATION.id,
    elements: document.querySelectorAll('#host *').length,
    textLength: (host.innerText || '').replace(/\s+/g, '').length,
    dataSection: document.querySelectorAll('#host [data-section]').length,
    dataEntry: document.querySelectorAll('#host [data-entry]').length,
    offerControls: offerButtons.length,
    offerControlsLive: offerButtons.filter((b) => !b.disabled).length,
    offerControlsTitled: offerButtons.every((b) => (b.title || '').length > 20),
    reachControls: document.querySelectorAll('#host [data-role="reach"]').length,
    menusOpen: document.querySelectorAll('#host [data-role="menu"]').length,
  };
  window.__gxNoticeMountSmoke = census;
  // What the page under a real pointer needs to answer questions about itself. Every
  // one of these reads the live document rather than remembering anything.
  window.__gxNoticeProbe = {
    menus: () => document.querySelectorAll('[data-role="menu"]').length,
    menuBox: () => {
      const menu = document.querySelector('[data-role="menu"]');
      if (!menu) return null;
      const r = menu.getBoundingClientRect();
      return {
        entry: menu.getAttribute('data-menu-entry'),
        offers: menu.querySelectorAll('[data-role="offer"]').length,
        disabled: [...menu.querySelectorAll('[data-role="offer"]')].filter((b) => b.disabled).length,
        left: Math.round(r.left),
        top: Math.round(r.top),
        right: Math.round(r.right),
        bottom: Math.round(r.bottom),
        insideViewport: r.left >= 0 && r.top >= 0 && r.right <= window.innerWidth && r.bottom <= window.innerHeight,
        saysNoActs: /nothing on this screen can be sent/.test(menu.textContent || ''),
      };
    },
    rowRect: (nth) => {
      const row = document.querySelectorAll('#host [data-role="entry"]')[nth];
      if (!row) return null;
      const r = row.getBoundingClientRect();
      return { x: Math.round(r.left + 40), y: Math.round(r.top + r.height / 2), entry: row.getAttribute('data-menu-row') };
    },
    // A row the open menu is not sitting on top of. Picking one by index and hoping
    // is how this smoke first "proved" that a second right-click did not move the
    // menu: the coordinates of the row it picked were underneath the menu already
    // standing there, so the press landed on the menu. It did find a real defect
    // doing it -- the menu answered as its own row -- and the way not to depend on
    // that accident again is to choose a row that is actually visible.
    rowAwayFromMenu: () => {
      const menu = document.querySelector('[data-role="menu"]');
      const over = menu ? menu.getBoundingClientRect() : null;
      const subject = menu ? menu.getAttribute('data-menu-entry') : null;
      for (const row of document.querySelectorAll('#host [data-role="entry"]')) {
        const r = row.getBoundingClientRect();
        if (row.getAttribute('data-menu-row') === subject) continue;
        if (over && r.bottom > over.top && r.top < over.bottom && r.right > over.left && r.left < over.right) continue;
        return { x: Math.round(r.left + 40), y: Math.round(r.top + r.height / 2), entry: row.getAttribute('data-menu-row') };
      }
      return null;
    },
    menuPoint: () => {
      const menu = document.querySelector('[data-role="menu"]');
      if (!menu) return null;
      const r = menu.getBoundingClientRect();
      return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + 8) };
    },
    gutterRect: (nth) => {
      const button = document.querySelectorAll('#host [data-shape="gutter"]')[nth];
      if (!button) return null;
      const r = button.getBoundingClientRect();
      return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), w: Math.round(r.width), h: Math.round(r.height) };
    },
    copyState: () => {
      const took = document.querySelectorAll('[data-copied="true"]').length;
      const failed = document.querySelectorAll('[data-copy-failed="true"]').length;
      const marked = document.querySelector('[data-copied="true"],[data-copy-failed="true"]');
      return { took, failed, says: marked ? (marked.textContent || '').trim().slice(0, 60) : null };
    },
    // The window records another call while a menu is open -- the ordinary path on
    // this face, and the one req/103 finding 2 was found through. What this returns is
    // how many menus stand in the document after the whole screen was rebuilt.
    grow: () => {
      record.push({ seq: record.length + 1, at: '2026-08-24T11:00:09.000Z', through: 'shell', method: 'theme:set', outcome: 'asked' });
      unmount.repaint();
      return document.querySelectorAll('[data-role="menu"]').length;
    },
  };
  unmount.repaint();
  document.title = `glovrex notice face -- ${requestedTheme === 'light' ? 'light' : 'dark'} -- elements ${census.elements} entries ${census.dataEntry}`;
}

run().catch((error) => {
  window.__gxNoticeMountSmoke = { ready: false, error: String((error && error.stack) || error) };
});
