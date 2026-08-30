// SPDX-License-Identifier: Apache-2.0
// req/38 SS24k: static screenshots are not design verification. This drives every
// disclosure this face has in a real module-loaded page (the same browser-mount
// fixture the W15 smoke uses) and captures a screenshot after each act, so the
// result is judged from pixels rather than assumed from the DOM.
//
// The gap this file closes is named in this face's own README ("`[ ]` No
// interaction pass"): the four control disclosures (why/legend/claims/omitted)
// and, separately, a subject's own fold were never individually pressed and shot.
// The right-click menu already has its own real-window pass -- see
// `tools/browser-mount-smoke.mjs`'s `menuPass()` (Owner #348 (2)) -- so this file
// does not repeat it; it presses the two regions that pass never touched.
//
// Two independence questions this pass asks and answers from pixels, not from
// reading the source: does opening one control leave the others' open state
// alone (receipt already asked this of its own two controls; this face has four,
// not two, and that is itself worth measuring rather than assuming carries over),
// and does opening a *subject's* own fold leave every control AND every other
// subject's own fold alone. Those are two different regions of this screen
// (`[data-role="control"]` and `[data-role="subject"]`) built by two different
// functions (`controlToggle()` and `subjectBox()`); nothing before this file
// checked they do not step on each other in a real document.
//
//   node faces/atlas/tools/interaction-pass.mjs

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startRenderer } from '../../../tools/rig/renderer.mjs';
import { writeFixture, startStaticServer, ROOT } from './browser-mount-smoke.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RECORD_DIR = path.resolve(HERE, '../record');

// SS857 (2026-08-29): a no-op is exactly the case a naive before/after compare
// cannot see -- it always reports "same" whether nothing ran or nothing changed.
// Every shot this file writes goes through here so the report can name any two
// shots that turned out byte-identical, rather than trusting that a different
// selector or a different click produced a different picture.
const shotHashes = [];
function writeShot(name, buffer) {
  fs.writeFileSync(path.join(RECORD_DIR, name), buffer);
  shotHashes.push({ shot: name, sha256: crypto.createHash('sha256').update(buffer).digest('hex') });
}

const settle = (page) => page.evaluate('new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))');

// `tools/rig/renderer.mjs`'s `capture()` clips a fixed rect at the page's own
// origin (x:0, y:0) with `captureBeyondViewport: false` -- faces/receipt's own
// interaction-pass.mjs already names half of this ("anything below the first
// 900px is not in a shot however far the page is scrolled") and its own next
// clause names the trap this file first walked straight into anyway:
// "scrolling first makes it worse, not better: the top stops being composited
// and the shot comes back blank cream". Measured here, not assumed from that
// comment: a first version of this file called `scrollIntoView()` before every
// capture and three of its nine shots (different acts, different
// window.scrollY, different DOM state, checked with `sha256`) came back
// byte-identical -- rendered, saved, opened, and blank. The fix receipt's own
// file already uses is the one applied here: never scroll, and open the
// renderer tall enough that every act this pass performs stays inside the one
// viewport the fixed clip actually captures.
const VIEWPORT = { width: 900, height: 2200 };

async function run() {
  fs.mkdirSync(RECORD_DIR, { recursive: true });
  const fixture = writeFixture();
  const server = await startStaticServer(ROOT);
  const { port } = server.address();
  const renderer = await startRenderer({ viewport: VIEWPORT });
  const acts = [];
  try {
    const page = await renderer.openPage();
    await page.raw.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'light' }] });
    const relative = path.relative(ROOT, path.join(fixture.dir, fixture.page)).split(path.sep).join('/');
    await page.open(`http://127.0.0.1:${port}/${relative}`);
    await page.hold('window.__gxMountSmoke !== undefined');

    // This fixture's own entry script (`browser-mount-smoke.mjs`'s ENTRY_SOURCE)
    // runs its own `menuPass()` unconditionally on load and leaves the right-click
    // menu open on purpose ("the last one is left open on purpose, so the shot has
    // a menu in it" -- that file's own comment) -- found here by looking at the
    // act 0 screenshot before trusting it, not by reading that comment first: a
    // "copy value" menu was already open, over the second subject row, before this
    // file had clicked anything. A real click-away (document body) is this screen's
    // own documented way to close it (`clickAwayClosed`, pinned in that same file),
    // so this is not a special case invented for this pass.
    await page.evaluate("document.body.dispatchEvent(new PointerEvent('pointerdown', { clientX: 2, clientY: 2, bubbles: true }))");
    await settle(page);
    const menuOpenAtStart = await page.evaluate("Boolean(document.querySelector('#host [data-part=\"row-menu\"]'))");

    // The same repaint witness receipt's own pass plants: mount() paints exactly
    // twice on this face too (waiting, then answered) and never again -- see
    // README's "Panel-open state across a repaint" section, which states the
    // mechanism but never planted this to check it from a live document.
    await page.evaluate("document.querySelector('#host [data-face]').__gxWitness = 'interaction-pass'");

    writeShot('atlas_act_0_initial.png', await page.capture());
    const initialSubjects = JSON.parse(await page.evaluate(`JSON.stringify(
      [...document.querySelectorAll('#host [data-role="subject"]')].map((d) => ({
        path: d.getAttribute('data-path'), open: d.open,
      }))
    )`));
    acts.push({
      act: 'initial', subjects: initialSubjects, shot: 'atlas_act_0_initial.png',
      note: 'the fixture entry script\'s own menuPass() runs on load and leaves a menu open; this pass dismisses it (real click-away) before treating this as "initial"',
      menuOpenAtStart,
    });

    // -- the four control disclosures, opened one at a time, each checked for not
    // having moved anything already open ----------------------------------------

    const controlOpen = (name) => page.evaluate(`(document.querySelector('#host [data-control="${name}"]') || {}).open ?? null`);
    const openControl = async (name, shotName) => {
      await page.evaluate(`document.querySelector('#host [data-control="${name}"] > summary').click()`);
      await settle(page);
      const state = await controlOpen(name);
      writeShot(shotName, await page.capture());
      return state;
    };
    const controlNames = JSON.parse(await page.evaluate(
      "JSON.stringify([...document.querySelectorAll('#host [data-role=\"control\"]')].map((d) => d.getAttribute('data-control')))",
    ));
    acts.push({ act: 'controls found', names: controlNames });

    const whyOpen = await openControl('why', 'atlas_act_1_why-open.png');
    acts.push({ act: 'why:open', open: whyOpen, shot: 'atlas_act_1_why-open.png' });

    const legendOpen = await openControl('legend', 'atlas_act_2_legend-open.png');
    const whyAfterLegend = await controlOpen('why');
    acts.push({
      act: 'legend:open', open: legendOpen, whyStillOpen: whyAfterLegend, shot: 'atlas_act_2_legend-open.png',
    });

    const claimsPresent = controlNames.includes('claims');
    let claimsOpen = null;
    if (claimsPresent) {
      claimsOpen = await openControl('claims', 'atlas_act_3_claims-open.png');
      acts.push({
        act: 'claims:open',
        open: claimsOpen,
        whyStillOpen: await controlOpen('why'),
        legendStillOpen: await controlOpen('legend'),
        shot: 'atlas_act_3_claims-open.png',
      });
    } else {
      acts.push({ act: 'claims:absent on this fixture (record.listOutcome !== "answered")', open: null });
    }

    const omittedOpen = await openControl('omitted', 'atlas_act_4_omitted-open.png');
    acts.push({
      act: 'omitted:open',
      open: omittedOpen,
      whyStillOpen: await controlOpen('why'),
      legendStillOpen: await controlOpen('legend'),
      claimsStillOpen: claimsPresent ? await controlOpen('claims') : null,
      shot: 'atlas_act_4_omitted-open.png',
    });

    // return guarantee: closing one control must not touch the others (the same
    // property receipt's own pass pinned of its two; this face has four)
    await page.evaluate("document.querySelector('#host [data-control=\"why\"] > summary').click()");
    await settle(page);
    const whyClosedAgain = await controlOpen('why');
    const legendAfterWhyClose = await controlOpen('legend');
    const omittedAfterWhyClose = await controlOpen('omitted');
    writeShot('atlas_act_5_why-closed.png', await page.capture());
    acts.push({
      act: 'why:close', whyClosedAgain, legendAfterWhyClose, omittedAfterWhyClose, shot: 'atlas_act_5_why-closed.png',
    });

    // -- a subject's own fold, which no earlier pass on this face has pressed ----

    const subjectPaths = initialSubjects.map((s) => s.path);
    const closedAtStart = initialSubjects.filter((s) => s.open === false).map((s) => s.path);
    const target = closedAtStart.length > 0 ? closedAtStart[0] : subjectPaths[0];
    const targetWasClosedAtStart = closedAtStart.includes(target);
    if (!targetWasClosedAtStart) {
      // Every subject this fixture ships was already open before this pass ever
      // ran, which needsOpen() would only do for a genuine hole or an overrun
      // verdict word -- reported plainly rather than silently forced into the
      // "closed -> open" shape this pass otherwise demonstrates.
      acts.push({ act: 'subject-fold:no subject was closed at start on this fixture', target });
    }

    const subjectOpen = (subjPath) => page.evaluate(
      `(document.querySelector('#host [data-role="subject"][data-path=${JSON.stringify(subjPath)}]') || {}).open ?? null`,
    );
    const clickSubjectFold = (subjPath) => page.evaluate(
      `document.querySelector('#host [data-role="subject"][data-path=${JSON.stringify(subjPath)}] [data-role="subject-summary"]').click()`,
    );

    await clickSubjectFold(target);
    await settle(page);
    const targetOpenAfterClick = await subjectOpen(target);
    const otherSubjectsUnchanged = [];
    for (const s of initialSubjects) {
      if (s.path === target) continue;
      const now = await subjectOpen(s.path);
      otherSubjectsUnchanged.push({ path: s.path, before: s.open, after: now, unchanged: now === s.open });
    }
    const controlsUnchangedAfterFold = {
      why: (await controlOpen('why')) === whyClosedAgain,
      legend: (await controlOpen('legend')) === legendAfterWhyClose,
      omitted: (await controlOpen('omitted')) === omittedAfterWhyClose,
    };
    writeShot('atlas_act_6_subject-open.png', await page.capture());
    acts.push({
      act: 'subject-fold:open', target, targetWasClosedAtStart, targetOpenAfterClick,
      otherSubjectsUnchanged, controlsUnchangedAfterFold, shot: 'atlas_act_6_subject-open.png',
    });

    // fold it back shut -- the same return guarantee, on the region no earlier
    // pass on this face pressed at all
    await clickSubjectFold(target);
    await settle(page);
    const targetOpenAfterSecondClick = await subjectOpen(target);
    writeShot('atlas_act_7_subject-closed.png', await page.capture());
    acts.push({
      act: 'subject-fold:close', target, targetOpenAfterSecondClick, shot: 'atlas_act_7_subject-closed.png',
    });

    // independence between two different subjects' own folds, not only between a
    // subject's fold and the four controls: open a second subject (if this
    // fixture has one) and check the first stays exactly as act 7 left it
    let secondSubject = null;
    if (subjectPaths.length > 1) {
      secondSubject = subjectPaths.find((p) => p !== target) ?? null;
    }
    if (secondSubject) {
      await clickSubjectFold(secondSubject);
      await settle(page);
      const secondOpenAfterClick = await subjectOpen(secondSubject);
      const targetStillClosed = await subjectOpen(target);
      writeShot('atlas_act_8_second-subject-open.png', await page.capture());
      acts.push({
        act: 'subject-fold:open a second, different subject',
        secondSubject, secondOpenAfterClick, targetStillClosed: targetStillClosed === targetOpenAfterSecondClick,
        shot: 'atlas_act_8_second-subject-open.png',
      });
    } else {
      acts.push({ act: 'subject-fold:only one subject on this fixture -- the two-subject independence check does not apply', subjectCount: subjectPaths.length });
    }

    const survived = await page.evaluate("document.querySelector('#host [data-face]').__gxWitness === 'interaction-pass'");
    const finalControlStates = {};
    for (const name of controlNames) finalControlStates[name] = await controlOpen(name);
    const finalSubjectStates = JSON.parse(await page.evaluate(`JSON.stringify(
      [...document.querySelectorAll('#host [data-role="subject"]')].map((d) => ({
        path: d.getAttribute('data-path'), open: d.open,
      }))
    )`));

    // SS857 self-check: any two shots this run wrote that came out byte-identical,
    // named rather than assumed absent. An earlier version of this file scrolled
    // before each capture and three shots came back byte-identical -- blank,
    // rendered on top of a compositor that had stopped painting under a scrolled
    // viewport (see the VIEWPORT comment above). This is the check that would
    // have caught it without a human reaching for sha256 by hand.
    const bySha = new Map();
    for (const { shot, sha256 } of shotHashes) {
      if (!bySha.has(sha256)) bySha.set(sha256, []);
      bySha.get(sha256).push(shot);
    }
    const duplicateShotGroups = [...bySha.values()].filter((group) => group.length > 1);

    const report = {
      what: 'req/38 SS24k interaction pass: every disclosure on faces/atlas pressed, screenshot after each act -- the two regions (control-row and subject fold) this face\'s own README named as not yet pressed',
      why: 'this face declares zero acts (read-only by design, ACTS = []); its interactivity is four native <details> control disclosures (why/legend/claims/omitted) plus, separately, every subject\'s own fold, and SS24k requires those be exercised and judged from pixels rather than assumed static. The right-click menu is not repeated here -- it already has its own real-window pass in tools/browser-mount-smoke.mjs',
      how: 'tools/rig/renderer.mjs against the same browser-mount fixture the W15 smoke drives, real module load, real click events on real <summary> elements',
      acts,
      shotIntegrity: {
        question: 'did any two of this run\'s own screenshots come out byte-identical (SS857: a no-op capture always reports "same", whether nothing ran or nothing visibly changed)',
        shotCount: shotHashes.length,
        duplicateShotGroups,
        allDistinct: duplicateShotGroups.length === 0,
      },
      openState: {
        question: 'does a disclosure a reader opened survive a repaint, and do the control region and the subject-fold region ever move each other',
        answer: 'not applicable to the repaint question, and here is the mechanism rather than the assurance -- the same one README already states for this face',
        mechanism: 'mount() paints exactly twice and never again: once with the waiting screen, once when the two reads answer. This face declares no acts, holds no state a control can change, and has nothing that asks for a redraw -- the only thing a reader can do to this screen is fold one of the four controls or one subject\'s own history, which the browser does in place. Nothing rebuilds the tree, so there is no repaint for an open state to be lost to.',
        witnessSurvivedEveryAct: survived,
        finalControlStates,
        finalSubjectStates,
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
