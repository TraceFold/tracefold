// SPDX-License-Identifier: Apache-2.0
// Every control in the shipped window, pressed, with the answer measured. req/810 §3.
//
//   node tools/btn_verify.mjs --origin http://127.0.0.1:8807
//
// The claim this closes is Owner #366's 「全Buttonを押したときの挙動をVerify」, and the
// reason it is a gate rather than an audit is that an audit is a sentence about one
// afternoon. req/103 drove 101 real operations and found real defects, and it covered two
// faces of six; the other four had zero audited operations and looked exactly as healthy.
//
// Four disciplines, each of which this file would be worthless without:
//
//   1. DENOMINATOR FIRST. The census is taken and printed before anything is pressed, so
//      "12 of 12 controls answered" cannot quietly become the report when the window drew
//      40. A control the census misses is itself a finding.
//   2. THE REAL INPUT PIPELINE. `Input.dispatchMouseEvent`, never a page-script
//      `dispatchEvent` -- req/103 §1 measured that synthetic events skip native
//      activation, and req/811 §8-2a measured a probe reporting a false absence because
//      it could only see inline handlers. A press that is not a press proves nothing.
//   3. A RESPONSE, OR A STATED REASON. A control answers if the document changed, or if
//      it is honestly disabled with a `why` -- the contract req/811 §8-7 put on refusals.
//      Silence with neither is the finding: a control that looks pressable and does
//      nothing is the defect this whole file exists to catch.
//   4. A NEGATIVE CONTROL, RUN EVERY TIME. `--plant` puts a dead button into the window
//      and requires this gate to go RED for it. A gate nobody has seen fail is not a gate,
//      and the way that is usually discovered is far too late.

import { startRenderer } from '../../tools/rig/renderer.mjs';

const argOf = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? fallback : process.argv[at + 1];
};

const ORIGIN = argOf('--origin', 'http://127.0.0.1:8807');
const PLANT = process.argv.includes('--plant');

/**
 * What counts as a control. Kept as one string so the census and any future gate read the
 * same definition, and so widening it is a visible edit rather than a drifting habit.
 */
export const CONTROL_SELECTOR = [
  'button', '[role="button"]', 'summary', '[data-act]', '[tabindex]', 'a[href]', 'input', 'select',
].join(',');

export const VERIFY_MESSAGES = {
  SILENT: 'pressed, and nothing in the document changed and no reason was given',
  CENSUS_EMPTY: 'the census found no controls at all, which is a broken census and not a calm window',
  PLANT_SURVIVED: 'the planted dead button was not caught, so this gate cannot see what it is for',
};

const renderer = await startRenderer({ viewport: { width: 1440, height: 900 } });
const page = await renderer.openPage();

const errors = [];
page.raw.on('Runtime.consoleAPICalled', (event) => {
  if (event.type === 'error') errors.push((event.args ?? []).map((a) => a.value ?? a.description).join(' '));
});
page.raw.on('Runtime.exceptionThrown', (event) => {
  errors.push(event.exceptionDetails?.exception?.description ?? 'uncaught');
});
await page.raw.send('Runtime.enable');

const evaluate = async (expression) => {
  const got = await page.raw.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (got.exceptionDetails) throw new Error(`${expression} -> ${got.exceptionDetails.text}`);
  return got.result?.value;
};
const frame = () => evaluate('new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))');

/**
 * A fresh window, with every control tagged by its position in document order.
 *
 * One window pressed 53 times is not 53 measurements. The second draft of this file did
 * that, and an early press that closed a dock took eighteen controls out of the document
 * with it -- so they were reported as "retired", never pressed, and the run still printed
 * GREEN over 53. A denominator of 53 with a dozen presses behind it is exactly the
 * dishonesty §3's denominator-first rule exists to stop, committed by the gate itself.
 *
 * So each control gets its own window, opened from the same address in the same state.
 * It costs one page load per control and buys the only thing that makes the number mean
 * anything: every control is measured against the same starting document.
 */
async function freshWindow() {
  await page.open(`${ORIGIN}/app.html`);
  await page.hold('document.documentElement.dataset.bound !== undefined');
  await page.settle();
  if (PLANT) {
    await evaluate(`(() => {
      const dead = document.createElement('button');
      dead.type = 'button';
      dead.className = 'planted-dead-button';
      dead.textContent = 'planted dead button';
      document.querySelector('.strip').append(dead);
      return true;
    })()`);
  }
  return JSON.parse(await evaluate(`(() => {
    const held = [...document.querySelectorAll(${JSON.stringify(CONTROL_SELECTOR)})];
    return JSON.stringify(held.map((el, index) => {
      el.dataset.verifyAt = String(index);
      const box = el.getBoundingClientRect();
      return {
        at: index,
        tag: el.tagName.toLowerCase(),
        label: (el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 46),
        region: el.closest('.topbar') ? 'topbar' : el.closest('.strip') ? 'strip'
          : el.closest('.dock') ? 'dock' : el.closest('.stage') ? 'stage' : 'other',
        disabled: el.disabled === true,
        why: el.dataset.why ?? el.getAttribute('title') ?? null,
        // Asked of the browser, not of the box -- for the reason written out at 'drawn'
        // below. The census keeps the same definition so the two cannot drift.
        visible: typeof el.checkVisibility === 'function'
          ? el.checkVisibility()
          : box.width > 0 && box.height > 0,
        x: Math.round(box.left + box.width / 2),
        y: Math.round(box.top + box.height / 2),
      };
    }));
  })()`));
}

/** The census. Taken first, printed first. */
const census = await freshWindow();

console.log(`census: ${census.length} controls in the shipped window`);
for (const [region, count] of Object.entries(census.reduce((into, c) => ({ ...into, [c.region]: (into[c.region] ?? 0) + 1 }), {}))) {
  console.log(`  ${region.padEnd(8)} ${count}`);
}
if (census.length === 0) {
  console.log(VERIFY_MESSAGES.CENSUS_EMPTY);
  await renderer.stop();
  process.exit(1);
}

const findings = [];
let answered = 0;
let excused = 0;

let pressed = 0;

for (const control of census) {
  // A fresh window per control, then this control re-read through its stable handle.
  //
  // The first draft pressed the coordinates taken during the census and reported 26 silent
  // controls -- every press after the first few had landed wherever that point happened to
  // be after the window moved, which is req/810 §3(c)'s "stale-handle no-ops are findings"
  // committed by the instrument. A gate that manufactures findings is worse than no gate,
  // because someone will go and fix them.
  await freshWindow();
  const now = JSON.parse(await evaluate(`(() => {
    const el = document.querySelector('[data-verify-at="${control.at}"]');
    if (!el) return JSON.stringify({ gone: true });

    // Two different facts wore one word until this block existed.
    //
    // The first draft called anything below the window "out-of-reach", and by that
    // reading a long dock is a defect and every scrolling list ever shipped is broken.
    // The second draft would have fixed the number by deleting the check, which is the
    // repair that goes green by asking an easier question -- req/819 §3 records the
    // census refusing exactly that trade.
    //
    // So the question asked here is the one a person's hand answers: is this control
    // inside a region that scrolls? If it is, scroll that region the way a reader would
    // and measure where the control then sits -- reachable, and reported as reachable.
    // If nothing in its ancestry scrolls, no amount of scrolling brings it back, and
    // that is the finding this gate was built to catch. The scroll that was performed is
    // carried into the report, so a run can never quietly turn the first fact into the
    // second without saying it moved something first.
    let scrolled = null;
    const scroller = (() => {
      for (let p = el.parentElement; p; p = p.parentElement) {
        const cs = getComputedStyle(p);
        const scrolls = cs.overflowY === 'auto' || cs.overflowY === 'scroll';
        if (scrolls && p.scrollHeight > p.clientHeight + 1) return p;
      }
      return null;
    })();
    const first = el.getBoundingClientRect();
    // Clipped against the SCROLLER, not against the window.
    //
    // The rule here used to be 'first.bottom > innerHeight || first.top < 0', which asks
    // whether the control is off the SCREEN. A control can be entirely on-screen and
    // still be cut off by the box it lives in, and this window has three of those.
    // Measured at 1440x900, in a fresh window per control (positions are quoted from
    // that and not from a sequential probe, where an earlier scrollIntoView moves every
    // control measured after it -- 'undo' reads y=704 fresh and y=477 after 'escalate'
    // has been scrolled to, and only the first number is about this window):
    //
    //   the 'reference' summary in the dock  y=798, .dock-host client box 601..779
    //   'escalate'                           y=525, .dock-host client box  82..515
    //   'undo'                               y=704, .dock-host client box  82..515
    //
    // Every one of those is under 900, so the old rule scrolled nothing. The hit test
    // for 'reference' then landed at 818,798 on strip-checks, and the gate reported a
    // placement defect against a control a reader reaches with one flick of the wheel;
    // scrolled, it sits at 818,690 and the hit test returns the summary itself.
    // 'escalate' and 'undo' turn out to be disabled and carrying a why -- which the old
    // rule never reached, because it failed them on placement two checks earlier.
    //
    // This makes the gate STRICTER and not looser: it now spends a reader's scroll on
    // regions it used to walk past, so anything that still reaches a finding has been
    // asked the harder question first. The scroller's client box is used rather than its
    // border box, because a border is not somewhere a pointer can land either.
    const view = (() => {
      if (!scroller) return null;
      const r = scroller.getBoundingClientRect();
      const top = r.top + scroller.clientTop;
      return { top, bottom: top + scroller.clientHeight };
    })();
    const clipped = Boolean(view) && (first.bottom > view.bottom || first.top < view.top);
    if (scroller && (clipped || first.bottom > innerHeight || first.top < 0)) {
      const was = scroller.scrollTop;
      el.scrollIntoView({ block: 'center' });
      scrolled = { by: String(scroller.className).slice(0, 30), from: was, to: scroller.scrollTop };
    }

    const box = el.getBoundingClientRect();
    const x = Math.round(box.left + box.width / 2);
    const y = Math.round(box.top + box.height / 2);
    // Would a click here actually land on this control? A control laid out below the
    // window, or under something else, is not a dead handler -- and calling it one sends
    // somebody to debug a handler that is fine. Asked with the browser's own hit test.
    const hit = document.elementFromPoint(x, y);

    // Whether the browser considers this drawn -- asked of the browser, not of the box.
    //
    // 'box.width > 0 && box.height > 0' was the old test, and the box stopped being able
    // to answer that question. Chrome hides the contents of a closed <details> with
    // content-visibility on ::details-content: the subtree is LAYOUT-SKIPPED, not
    // display:none, so getBoundingClientRect() hands back whatever numbers it last had
    // and they are garbage. Measured at 1440x900 against this window: the three
    // 'internal reference' summaries inside closed <details data-role=internal-reference>
    // report centres at y=428, y=24078 and y=14923, carrying entirely plausible 228x40,
    // 198x40 and 145x60 boxes, and div[data-role=legend] claims a height of 23856px.
    // Every one of those is non-zero, so the old test called all three visible and then
    // reported them as placement defects at coordinates that do not exist.
    // el.checkVisibility() returns false for all three.
    //
    // Measured present in this renderer -- typeof Element.prototype.checkVisibility is
    // 'function' -- so the fallback is not dead code kept on a guess; it is one
    // expression, and it is there because the degraded answer should be the old honest
    // geometry rather than a gate that quietly stops asking.
    const drawn = typeof el.checkVisibility === 'function'
      ? el.checkVisibility()
      : box.width > 0 && box.height > 0;

    // When it is not drawn, WHY it is not drawn -- because two of the reasons are the
    // window behaving correctly and one of them is a defect, and only a stated reason
    // can tell them apart. Both of these are checkable by a reader: open the disclosure,
    // or invoke the palette, and the control is there.
    //
    // hiddenAncestor is asked first on purpose. display:none is absolute: a closed
    // disclosure inside a hidden palette is not one press away, it is zero presses from
    // anywhere until the palette is invoked. Measured here the two are disjoint (#53 has
    // a hidden ancestor and no closed disclosure; #20/#22/#24 the reverse), so the order
    // decides nothing today and would decide correctly the day it does.
    const hiddenAncestor = (() => {
      for (let p = el; p; p = p.parentElement) {
        const cs = getComputedStyle(p);
        if (cs.display === 'none' || p.hidden === true) return String(p.className || p.tagName).slice(0, 40);
      }
      return null;
    })();
    // NOT sufficient on its own, and this is the trap in it: a <summary> is a child of
    // its own <details>, so closest('details:not([open])') matches every summary of
    // every closed disclosure in the window -- including the dock's 'reference' summary,
    // which is closed, matches, and is perfectly visible (measured: checkVisibility() is
    // true). Read this on its own and three of the dock's controls get excused for being
    // exactly what they are supposed to be. So it is only ever consulted after 'drawn'
    // has already come back false.
    const closedDisclosure = el.closest('details:not([open])') !== null;

    return JSON.stringify({
      gone: false,
      disabled: el.disabled === true,
      why: el.dataset.why ?? el.getAttribute('title') ?? null,
      drawn,
      zeroSize: box.width === 0 || box.height === 0,
      hiddenAncestor,
      closedDisclosure,
      inWindow: y >= 0 && y <= window.innerHeight && x >= 0 && x <= window.innerWidth,
      hittable: Boolean(hit && (hit === el || el.contains(hit) || el.contains(hit.parentElement))),
      hitInstead: hit ? String(hit.className || hit.tagName).slice(0, 40) : 'nothing',
      scrolled,
      hasScroller: Boolean(scroller),
      x,
      y,
    });
  })()`));

  if (now.gone) {
    // With a fresh window per control this should not happen at all; if it does, the
    // window is not opening the same way twice, which is a finding about the product.
    findings.push({ at: control.at, label: control.label, kind: 'not-reproduced', said: 'this control was in the census and is not in a freshly opened window' });
    continue;
  }
  if (!now.drawn) {
    // Three different facts wore one word here, and two of them are not defects.
    //
    // The old line called every one of these `unreachable`, which is a sentence about
    // the window that is not true: a control behind a closed disclosure is one press
    // from a reader's hand, and a palette that is not resident is req/811 §8-5 working.
    // A census that calls either of those unreachable is lying about the window in the
    // direction that looks rigorous, which is the worse direction to lie in.
    //
    // Both excuses are named, counted, and printed with their count -- never dropped --
    // so the report cannot shrink by reclassifying. What it cannot do is turn the run
    // RED, because there is nothing here for anyone to go and fix.
    if (now.hiddenAncestor) {
      findings.push({
        at: control.at,
        label: control.label,
        kind: 'not-yet-invoked',
        said: `not drawn because \`${now.hiddenAncestor}\` is display:none or [hidden]; it is invoked, not resident`,
      });
      continue;
    }
    if (now.closedDisclosure) {
      findings.push({
        at: control.at,
        label: control.label,
        kind: 'behind-a-closed-disclosure',
        said: 'not drawn because an ancestor <details> is closed, so it is one press away and was not pressed',
      });
      continue;
    }
    // Nothing in its ancestry accounts for it. That is the finding, and it stays RED.
    findings.push({
      at: control.at,
      label: control.label,
      kind: 'unreachable',
      said: 'the browser reports it as not drawn and nothing in its ancestry says why, so it was not pressed',
    });
    continue;
  }
  if (now.zeroSize) {
    // Drawn, and drawn at nothing. `checkVisibility()` answers display and content
    // visibility, and a zero-size box is neither of those -- it comes back true for a
    // 0x0 button. So the geometric test is kept, moved to where it is still correct,
    // and this is the one case the old line was actually good at. Still a defect, still
    // RED: a control with no area is a control no pointer can arrive at.
    findings.push({ at: control.at, label: control.label, kind: 'unreachable', said: 'the browser reports it as drawn, at zero size, so it was not pressed' });
    continue;
  }
  if (!now.inWindow || !now.hittable) {
    // A separate kind on purpose. This is a placement defect, not a dead control: the
    // handler may be perfect and no pointer can arrive at it. Reported with the numbers
    // that say which, so the reader is sent to the layout and not to the handler.
    findings.push({
      at: control.at,
      label: control.label,
      // The two facts, named apart. `out-of-reach` is a control no pointer can arrive
      // at by any means; `occluded-after-scroll` is one that was scrolled to and still
      // has something else on top of it, which is a real defect of a different kind and
      // is sent to a different place in the code. Only the first is what this gate was
      // built around, and both are still findings.
      //
      // Keyed off `scrolled` and not off `hasScroller`, which was the earlier reading
      // and was a claim about a scroll that had never happened. Measured: the first
      // `internal reference` summary sat at y=428 inside a `.pane-host` and passed the
      // old window clip untouched, so `scrolled` was null and nothing had moved -- and
      // the report still said `occluded-after-scroll`, on a line whose own text carried
      // no "after scrolling" clause to contradict it. Having a scroller is not the same
      // fact as having been scrolled, and the report may only say the second when it is
      // true. With the scroller clip above in place this window no longer produces a
      // witness for it; it is kept because the wrong reading was still being made.
      kind: now.scrolled ? 'occluded-after-scroll' : 'out-of-reach',
      said: now.inWindow
        ? `drawn at ${now.x},${now.y}`
          + (now.scrolled ? ` after scrolling ${now.scrolled.by} to ${now.scrolled.to}` : '')
          + ` but a click there lands on ${now.hitInstead}`
        : `laid out at ${now.x},${now.y}, outside a 1440x900 window`
          + (now.scrolled
            ? `, and scrolling ${now.scrolled.by} to ${now.scrolled.to} did not bring it in`
            : ', and nothing scrolled it into reach, so no pointer can arrive at it'),
    });
    continue;
  }
  if (now.disabled) {
    // A disabled control answers by carrying its reason. That IS the response.
    if (now.why && now.why.length > 3) { excused += 1; continue; }
    findings.push({ at: control.at, label: control.label, kind: 'no-reason', said: 'disabled and carries no why' });
    continue;
  }

  const before = await evaluate('document.documentElement.outerHTML.length + "|" + document.querySelectorAll("*").length');
  const errorsBefore = errors.length;
  for (const type of ['mousePressed', 'mouseReleased']) {
    await page.raw.send('Input.dispatchMouseEvent', { type, x: now.x, y: now.y, button: 'left', buttons: 1, clickCount: 1 });
  }
  await frame();
  const after = await evaluate('document.documentElement.outerHTML.length + "|" + document.querySelectorAll("*").length');

  pressed += 1;
  if (after !== before) answered += 1;
  else findings.push({ at: control.at, label: control.label, kind: 'silent', said: VERIFY_MESSAGES.SILENT });
  if (errors.length > errorsBefore) {
    findings.push({ at: control.at, label: control.label, kind: 'console', said: errors[errors.length - 1].slice(0, 120) });
  }
}

// The two kinds a reader can check for themselves, and the reason they are not RED:
// in both, the control is reachable and the window is doing what it was built to do.
// They are still findings, still counted, and still printed one line each with the rest
// -- an excuse that disappears from the report is just a deletion with better manners.
const EXCUSED_KINDS = ['behind-a-closed-disclosure', 'not-yet-invoked'];

// What turns the run RED. `out-of-reach` counts: a control a pointer cannot arrive at is
// a control that does not work, whatever its handler would have done. `unreachable`
// counts too, now that it means what it says -- a control the browser cannot account for
// is not a control anybody pressed.
const silent = findings.filter((f) => f.kind === 'silent' || f.kind === 'no-reason' || f.kind === 'out-of-reach' || f.kind === 'occluded-after-scroll' || f.kind === 'unreachable');
// Both numbers, never the first alone: how many were pressed is the denominator that
// makes "answered" mean anything.
console.log(`\npressed ${pressed} of ${census.length}: ${answered} answered with a change, ${excused} refused with a stated reason`);
// Printed even when every count is zero. A category that only appears when it is
// non-empty is a category a reader cannot tell the gate is still asking about.
const excusedFound = findings.filter((f) => EXCUSED_KINDS.includes(f.kind));
console.log(`excused: ${excusedFound.length} (${EXCUSED_KINDS.map((k) => `${k} ${findings.filter((f) => f.kind === k).length}`).join(', ')})`);
console.log(`findings: ${findings.length}`);
for (const found of findings) console.log(`  [${found.kind}] #${found.at} ${found.label} -- ${found.said}`);
console.log(`console errors: ${errors.length}`);

await renderer.stop();

if (PLANT) {
  // Judged inside out: with a dead button planted, a CLEAN report is the failure.
  const caught = silent.some((f) => f.label.includes('planted dead button'));
  console.log(`\nnegative control: the planted dead button was ${caught ? 'CAUGHT' : 'MISSED'}`);
  if (!caught) console.log(VERIFY_MESSAGES.PLANT_SURVIVED);
  process.exitCode = caught ? 0 : 1;
} else {
  const red = silent.length > 0 || errors.length > 0;
  console.log(`\nverdict: ${red ? 'RED' : 'GREEN'} over ${pressed} controls pressed of ${census.length} censused`);
  process.exitCode = red ? 1 : 0;
}
