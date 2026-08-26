// SPDX-License-Identifier: Apache-2.0
// T3 -- what the renderer actually painted.
//
// Three readings, because one of them is never enough (req/06 §4-3):
//   P-a  ink box overlap    text-run rectangles, not element rectangles
//   P-b  used size          what the glyph became, against what it declared
//   P-c  baseline ink       the binarised picture, against a committed one
//
// Each is answerable in numbers. None of them reads the stylesheet, because the
// defects these exist for are defects where the stylesheet is doing what it says.

import { decodePng, inkMask, maskDigest, compareMasks } from '../rig/raster.mjs';

export const PIXEL_MESSAGES = {
  OVERLAP_CLEAR: 'no two text runs share ink area',
  OVERLAP_FOUND: 'text runs overlap',
  SIZE_MATCHED: 'every glyph was used at the size it declares',
  SIZE_DRIFTED: 'glyphs were used at a size they do not declare',
  BASELINE_MATCHED: 'painted ink matches the committed baseline',
  BASELINE_DRIFTED: 'painted ink differs from the committed baseline',
  BASELINE_ABSENT: 'no committed baseline for this face -- a first capture is not a pass',
};

// The SVG replaced-element default. A glyph landing on it did not get sized; it got
// abandoned. Naming it explicitly keeps that failure from reading as a near miss.
const REPLACED_DEFAULT = { width: 300, height: 150 };

// ---------------------------------------------------------------- P-a

// Collected in the page because a text run is a Range, and a Range has no
// meaning out here. One rect per line box, not one per element.
const TEXT_RUN_RECTS = `(() => {
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const runs = [];
  let node;
  while ((node = walker.nextNode())) {
    if (!node.nodeValue || !node.nodeValue.trim()) continue;
    const style = getComputedStyle(node.parentElement);
    if (style.visibility === 'hidden' || style.display === 'none' || Number(style.opacity) === 0) continue;
    const range = document.createRange();
    range.selectNodeContents(node);
    for (const rect of range.getClientRects()) {
      if (rect.width <= 0 || rect.height <= 0) continue;
      runs.push({
        text: node.nodeValue.trim().slice(0, 60),
        owner: node.parentElement.tagName.toLowerCase() + (node.parentElement.className ? '.' + String(node.parentElement.className).split(' ').join('.') : ''),
        x: rect.x, y: rect.y, width: rect.width, height: rect.height,
      });
    }
  }
  return runs;
})()`;

const intersectionArea = (a, b) => {
  const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
  return w > 0 && h > 0 ? w * h : 0;
};

export async function inkBoxOverlap(page) {
  const runs = await page.evaluate(TEXT_RUN_RECTS);
  const overlaps = [];
  for (let i = 0; i < runs.length; i += 1) {
    for (let j = i + 1; j < runs.length; j += 1) {
      const area = intersectionArea(runs[i], runs[j]);
      if (area > 0) {
        overlaps.push({
          area: Number(area.toFixed(2)),
          a: { text: runs[i].text, owner: runs[i].owner },
          b: { text: runs[j].text, owner: runs[j].owner },
        });
      }
    }
  }
  overlaps.sort((p, q) => q.area - p.area);
  return {
    id: 'P-a',
    population: runs.length,
    findings: overlaps,
    ok: overlaps.length === 0,
    message: overlaps.length === 0
      ? PIXEL_MESSAGES.OVERLAP_CLEAR
      : `${PIXEL_MESSAGES.OVERLAP_FOUND}: ${overlaps.length} pair(s), largest ${overlaps[0].area}px2`,
  };
}

// ---------------------------------------------------------------- P-b

const GLYPH_USED_SIZES = `(() => {
  return Array.from(document.querySelectorAll('[data-glyph]')).map((el) => {
    const rect = el.getBoundingClientRect();
    return {
      name: el.getAttribute('data-glyph'),
      declared: el.getAttribute('data-glyph-size'),
      usedWidth: Number(rect.width.toFixed(3)),
      usedHeight: Number(rect.height.toFixed(3)),
    };
  });
})()`;

export async function usedGlyphSize(page) {
  const glyphs = await page.evaluate(GLYPH_USED_SIZES);
  const findings = [];
  for (const glyph of glyphs) {
    if (!glyph.declared) {
      findings.push({ ...glyph, why: 'glyph declares no size' });
      continue;
    }
    const [w, h] = glyph.declared.split('x').map(Number);
    if (glyph.usedWidth === 0 || glyph.usedHeight === 0) {
      findings.push({ ...glyph, why: 'used size collapsed to zero' });
    } else if (glyph.usedWidth === REPLACED_DEFAULT.width && glyph.usedHeight === REPLACED_DEFAULT.height) {
      findings.push({ ...glyph, why: `fell to the replaced-element default ${REPLACED_DEFAULT.width}x${REPLACED_DEFAULT.height}` });
    } else if (glyph.usedWidth !== w || glyph.usedHeight !== h) {
      findings.push({ ...glyph, why: `declared ${w}x${h}, used ${glyph.usedWidth}x${glyph.usedHeight}` });
    }
  }
  return {
    id: 'P-b',
    population: glyphs.length,
    findings,
    ok: findings.length === 0,
    message: findings.length === 0
      ? PIXEL_MESSAGES.SIZE_MATCHED
      : `${PIXEL_MESSAGES.SIZE_DRIFTED}: ${findings.length} of ${glyphs.length}`,
  };
}

// ---------------------------------------------------------------- P-c

export async function baselineInk(page, { baseline = null } = {}) {
  const png = await page.capture();
  const raster = decodePng(png);
  const ink = inkMask(raster);
  const digest = maskDigest(ink);
  const observed = { digest, width: ink.width, height: ink.height, inkPixels: ink.inkPixels };

  if (!baseline) {
    return {
      id: 'P-c', population: ink.inkPixels, observed, findings: [{ why: PIXEL_MESSAGES.BASELINE_ABSENT }],
      ok: false, message: PIXEL_MESSAGES.BASELINE_ABSENT,
    };
  }
  if (baseline.digest === digest) {
    return { id: 'P-c', population: ink.inkPixels, observed, findings: [], ok: true, message: PIXEL_MESSAGES.BASELINE_MATCHED };
  }
  let detail = `digest ${baseline.digest} -> ${digest}`;
  if (baseline.mask && baseline.width === ink.width && baseline.height === ink.height) {
    const diff = compareMasks({ width: baseline.width, height: baseline.height, mask: baseline.mask }, ink);
    detail = `${diff.differing} ink px differ (${(100 * diff.differing / diff.total).toFixed(4)}%), first at ${diff.firstAt?.x},${diff.firstAt?.y}`;
  }
  return {
    id: 'P-c', population: ink.inkPixels, observed,
    findings: [{ why: detail, baselineInkPixels: baseline.inkPixels, observedInkPixels: ink.inkPixels }],
    ok: false, message: `${PIXEL_MESSAGES.BASELINE_DRIFTED}: ${detail}`,
  };
}

export async function captureInk(page) {
  const raster = decodePng(await page.capture());
  const ink = inkMask(raster);
  return { digest: maskDigest(ink), width: ink.width, height: ink.height, inkPixels: ink.inkPixels, mask: ink.mask };
}
