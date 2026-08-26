// SPDX-License-Identifier: Apache-2.0
// req/38 SS24k: static screenshots are not design verification. This drives every
// interactive control this face has in a real module-loaded page (the same
// browser-mount fixture the W15 smoke uses) and captures a screenshot after each act,
// so the result is judged from pixels rather than assumed from the DOM.
//
// Two disclosures ("why" and "legend") and, since the r5 pass, the context menu:
// there are still no act buttons on this face (it is read-only by declaration,
// ACTS = []), and the menu's one entry hands a value over rather than changing
// anything. Owner #348 (2) asks for five properties to be pinned and each is an act
// below: the menu opens, it acts and states whether the act worked, Escape dismisses
// it, a click away dismisses it, and a second right-click does not stack two.
//
// The right-click is dispatched as a real `contextmenu` MouseEvent on a real element
// in a real browser, and travels the real bubbling path into the real listener. It is
// not an operating-system right-click through Input.dispatchMouseEvent, which would
// need screen coordinates -- and a test that aims at coordinates is a test that starts
// passing against the wrong element the moment a row moves.
//
//   node faces/receipt/tools/interaction-pass.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { writeFixture, startStaticServer, ROOT } from './browser-mount-smoke.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RECORD_DIR = path.resolve(HERE, '../record');

async function run() {
  fs.mkdirSync(RECORD_DIR, { recursive: true });
  const fixture = writeFixture();
  const server = await startStaticServer(ROOT);
  const { port } = server.address();
  const renderer = await startRenderer({ viewport: { width: 900, height: 900 } });
  const acts = [];
  try {
    const page = await renderer.openPage();
    await page.raw.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
    const relative = path.relative(ROOT, path.join(fixture.dir, fixture.page)).split(path.sep).join('/');
    await page.open(`http://127.0.0.1:${port}/${relative}`);
    await page.hold('window.__gxMountSmoke !== undefined');

    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_0_initial.png'), await page.capture());
    acts.push({ act: 'initial', shot: 'receipt_act_0_initial.png' });

    // The repaint question, asked of the live page rather than of the source: a
    // disclosure a reader has opened must not be shut by something else redrawing the
    // screen. `paint()` empties the host and appends a freshly built element, so a
    // property put on the drawn tree's root object cannot survive a repaint. Marking
    // it here and reading it back after every act is a witness, not a claim.
    await page.evaluate("document.querySelector('#host [data-face]').__gxWitness = 'r4'");

    // act 1: open "why"
    await page.evaluate("document.querySelectorAll('#host summary')[0].click()");
    await page.evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    const whyOpen = await page.evaluate("document.querySelectorAll('#host details')[0].open");
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_1_why-open.png'), await page.capture());
    acts.push({ act: 'why:open', open: whyOpen, shot: 'receipt_act_1_why-open.png' });

    // act 2: open "legend" (why stays open -- disclosures are independent, no
    // dismiss-layer ordering the way a modal/menu would have)
    await page.evaluate("document.querySelectorAll('#host summary')[1].click()");
    await page.evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    const legendOpen = await page.evaluate("document.querySelectorAll('#host details')[1].open");
    const whyStillOpen = await page.evaluate("document.querySelectorAll('#host details')[0].open");
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_2_legend-open.png'), await page.capture());
    acts.push({
      act: 'legend:open', open: legendOpen, whyStillOpen, shot: 'receipt_act_2_legend-open.png',
    });

    // The legend is folded shut in every fixture, so tools/shoot.mjs measures it as
    // invisible and reports no overlap in it -- which is true and useless. It is open
    // now, in a real renderer, so this is the one place its own geometry can be read:
    // a mark name wider than its track paints over the count beside it, which is what
    // r4 shipped and what the shot of this very act showed.
    const legendFit = await page.evaluate(`(() => {
      const names = [...document.querySelectorAll('#host [data-mark-entry] > span:first-child')];
      const over = names.filter((n) => n.scrollWidth > n.clientWidth + 1).map((n) => n.textContent);
      const lines = names.filter((n) => n.getBoundingClientRect().height > 24).map((n) => n.textContent);
      return JSON.stringify({ measured: names.length, overflowing: over, wrappedToTwoLines: lines });
    })()`);
    acts.push({ act: 'legend:measure the mark column', fit: JSON.parse(legendFit) });

    // act 3: close "why" again -- return guarantee: closing one disclosure must
    // not touch the other (req/09 §0-3's "zero dead ends" reading, applied here)
    await page.evaluate("document.querySelectorAll('#host summary')[0].click()");
    await page.evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    const whyClosedAgain = await page.evaluate("document.querySelectorAll('#host details')[0].open");
    const legendStillOpen = await page.evaluate("document.querySelectorAll('#host details')[1].open");
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_3_why-closed.png'), await page.capture());
    acts.push({
      act: 'why:close', whyClosedAgain, legendStillOpen, shot: 'receipt_act_3_why-closed.png',
    });

    // -- the menu (Owner #348 (2)) -------------------------------------------------

    const settle = () => page.evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');
    const rightClick = (selector) => page.evaluate(
      `document.querySelector(${JSON.stringify(selector)}).dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }))`,
    );
    const menuCount = () => page.evaluate("document.querySelectorAll('#host [data-part=\"menu\"]').length");
    const menuWords = () => page.evaluate("(document.querySelector('#host [data-part=\"menu\"]') || {}).textContent || ''");
    // The rig captures a fixed clip at the page's own origin, so anything below the
    // first 900px is not in a shot however far the page is scrolled -- scrolling first
    // makes it worse, not better: the top stops being composited and the shot comes
    // back blank cream, which is what the first version of this pass wrote to disk.
    // So the acts that have to be seen are performed on the cell at the top of the
    // screen, which is also the one a reader most often wants: the delta's own id. The
    // fingerprint cell, further down, is where the no-stacking and shortened-value
    // readings are taken, and those are numbers rather than pictures.
    const SUBJECT = '#host [data-role="subject"]';
    const FINGERPRINT = '#host [data-cell="fingerprint"]';
    const entryOutcome = () => page.evaluate(`(() => {
      const e = document.querySelector('#host [data-part="menu"] [data-entry]');
      return e ? JSON.stringify({
        copied: e.getAttribute('data-copied'),
        failed: e.getAttribute('data-copy-failed'),
        said: e.getAttribute('data-copy-said'),
      }) : 'null';
    })()`);

    // act 4: open the menu on the delta's id, at the top of the screen.
    await rightClick(SUBJECT);
    await settle();
    const openedCount = await menuCount();
    const openedWords = await menuWords();
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_4_menu-open.png'), await page.capture());
    acts.push({
      act: 'menu:open on the delta id', menus: openedCount, words: openedWords, shot: 'receipt_act_4_menu-open.png',
    });

    // act 5: a second right-click, on a different cell. One menu, not two -- and the
    // shot shows the first one gone from under the header, which is the same fact
    // seen rather than counted.
    await rightClick(FINGERPRINT);
    await settle();
    const afterSecond = await menuCount();
    const secondWords = await menuWords();
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_5_menu-second.png'), await page.capture());
    acts.push({
      act: 'menu:second right-click on the fingerprint cell', menus: afterSecond, stacked: afterSecond > 1, words: secondWords, shot: 'receipt_act_5_menu-second.png',
    });

    // act 6: press it, back on the id where it can be photographed. Whatever the
    // clipboard does, the entry states it.
    await rightClick(SUBJECT);
    await settle();
    await page.evaluate("document.querySelector('#host [data-part=\"menu\"] [data-entry]').click()");
    await settle();
    await settle();
    const said = await entryOutcome();
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_6_menu-acted.png'), await page.capture());
    acts.push({ act: 'menu:copy pressed', outcome: JSON.parse(said), shot: 'receipt_act_6_menu-acted.png' });

    // act 7: Escape, from the document, the way a reader presses it.
    await page.evaluate("document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))");
    await settle();
    const afterEscape = await menuCount();
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_7_menu-escaped.png'), await page.capture());
    acts.push({ act: 'menu:Escape', menus: afterEscape, shot: 'receipt_act_7_menu-escaped.png' });

    // act 8: open it again and click somewhere else entirely -- on this page, the
    // body outside the host, which only the document-level listener can hear.
    await rightClick(SUBJECT);
    await settle();
    const reopened = await menuCount();
    await page.evaluate("document.body.dispatchEvent(new MouseEvent('click', { bubbles: true }))");
    await settle();
    const afterAway = await menuCount();
    fs.writeFileSync(path.join(RECORD_DIR, 'receipt_act_8_menu-clicked-away.png'), await page.capture());
    acts.push({
      act: 'menu:click away', reopened, menus: afterAway, shot: 'receipt_act_8_menu-clicked-away.png',
    });

    const survived = await page.evaluate("document.querySelector('#host [data-face]').__gxWitness === 'r4'");
    const openAfter = await page.evaluate("[...document.querySelectorAll('#host details')].map((d) => d.open)");

    const report = {
      what: 'req/38 SS24k interaction pass: every visible control on faces/receipt pressed, screenshot after each act',
      why: 'this face declares zero acts (read-only by design); its interactivity is two native <details> disclosures and, since r5, a context menu whose one entry hands a value over, and SS24k requires those be exercised and judged from pixels rather than assumed static',
      how: 'tools/rig/renderer.mjs against the same browser-mount fixture the W15 smoke drives, real module load, real click events, real contextmenu MouseEvents dispatched on real elements',
      acts,
      menu: {
        question: 'does the menu open, act, say whether the act worked, dismiss two ways, and refuse to stack',
        pinned: ['open', 'act with a stated outcome', 'Escape', 'click away', 'no stacking'],
        stackedEver: acts.some((a) => a.stacked === true),
      },
      openState: {
        question: 'does a disclosure a reader opened survive a repaint',
        answer: 'not applicable on this face, and here is the mechanism rather than the assurance',
        mechanism: 'mount() paints exactly twice and never again: once with the waiting screen, once when the two reads answer. This face declares no acts, holds no state a control can change, and has nothing that asks for a redraw -- the only thing a reader can do to this screen is fold one of the two disclosures, which the browser does in place. Nothing rebuilds the tree, so there is no repaint for an open state to be lost to.',
        witnessSurvivedEveryAct: survived,
        openAfterEveryAct: openAfter,
      },
      readByEye: { held: [], notHeld: ['not yet judged by a person -- screenshots written, judged separately and recorded in README.md'] },
    };
    fs.writeFileSync(path.join(RECORD_DIR, 'interaction-pass.json'), `${JSON.stringify(report, null, 2)}\n`, 'utf8');
    return report;
  } finally {
    await renderer.stop();
    server.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  run().then((report) => process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)).catch((error) => {
    process.stderr.write(`${error.stack ?? error.message}\n`);
    process.exitCode = 1;
  });
}
