// SPDX-License-Identifier: Apache-2.0
// The shell's structural marks. Not the record sprite -- that one carries meaning
// (admit, deny, held) and belongs to the faces; these carry structure (this closes, this
// dock is on that side) and belong to the frame. Two namespaces, on purpose, and the gate
// checks the shell never reaches for a `g-` id and the faces never define a structural
// mark.
//
// Nothing borrowed: no icon font, no emoji, and none of the general-purpose glyphs a
// person can type. Every mark is drawn here, and every mark is drawn at a size it was
// given. An <svg> without width and height falls back to the format's default box --
// 300x150 -- and the first time that happens in a strip it takes the strip with it.

const NS = 'http://www.w3.org/2000/svg';

/** Each entry is a list of strokes in a 16-unit box. */
const MARKS = Object.freeze({
  close: [['M', 4.5, 4.5, 'L', 11.5, 11.5], ['M', 11.5, 4.5, 'L', 4.5, 11.5]],
  'dock-left': [['M', 2.5, 3.5, 'L', 13.5, 3.5, 'L', 13.5, 12.5, 'L', 2.5, 12.5, 'Z'], ['M', 6.5, 3.5, 'L', 6.5, 12.5]],
  'dock-right': [['M', 2.5, 3.5, 'L', 13.5, 3.5, 'L', 13.5, 12.5, 'L', 2.5, 12.5, 'Z'], ['M', 9.5, 3.5, 'L', 9.5, 12.5]],
  'dock-bottom': [['M', 2.5, 3.5, 'L', 13.5, 3.5, 'L', 13.5, 12.5, 'L', 2.5, 12.5, 'Z'], ['M', 2.5, 9.5, 'L', 13.5, 9.5]],
  'divide-row': [['M', 2.5, 3.5, 'L', 13.5, 3.5, 'L', 13.5, 12.5, 'L', 2.5, 12.5, 'Z'], ['M', 8, 3.5, 'L', 8, 12.5]],
  // The standing column's fourth region (req/811 §4-1 B-2): a face placed nowhere in this
  // space. It is the same enclosure the three region marks draw, left open -- the glyph
  // canon's existing "a space that was declared and left empty, as against a space nobody
  // mentioned". Drawn rather than hidden, because an unplaced face is an explicit row.
  nowhere: [
    ['M', 2.5, 3.5, 'L', 5.5, 3.5], ['M', 10.5, 3.5, 'L', 13.5, 3.5],
    ['M', 13.5, 3.5, 'L', 13.5, 6.5], ['M', 13.5, 9.5, 'L', 13.5, 12.5],
    ['M', 13.5, 12.5, 'L', 10.5, 12.5], ['M', 5.5, 12.5, 'L', 2.5, 12.5],
    ['M', 2.5, 12.5, 'L', 2.5, 9.5], ['M', 2.5, 6.5, 'L', 2.5, 3.5],
  ],
  'divide-col': [['M', 2.5, 3.5, 'L', 13.5, 3.5, 'L', 13.5, 12.5, 'L', 2.5, 12.5, 'Z'], ['M', 2.5, 8, 'L', 13.5, 8]],
  back: [['M', 10, 3.5, 'L', 5, 8, 'L', 10, 12.5]],
  forward: [['M', 6, 3.5, 'L', 11, 8, 'L', 6, 12.5]],
  space: [['M', 2.5, 4.5, 'L', 7, 4.5, 'L', 7, 11.5, 'L', 2.5, 11.5, 'Z'], ['M', 9, 4.5, 'L', 13.5, 4.5, 'L', 13.5, 11.5, 'L', 9, 11.5, 'Z']],
  theme: [['M', 8, 2.5, 'L', 8, 13.5], ['M', 8, 2.5, 'A', 5.5, 5.5, 0, 0, 1, 8, 13.5]],
  // SS551: the copy control on a per-object command block. Two overlapping
  // rectangles, the ordinary way "copy" is drawn without borrowing a glyph font.
  copy: [['M', 3.5, 3.5, 'L', 9.5, 3.5, 'L', 9.5, 9.5, 'L', 3.5, 9.5, 'Z'], ['M', 6.5, 6.5, 'L', 12.5, 6.5, 'L', 12.5, 12.5, 'L', 6.5, 12.5, 'Z']],
  // req/822_c5 item 1: a reading that is about another tree. The closed rectangle is the
  // report; the open corner strokes at the lower right are the tree it described, no
  // longer where the report left it -- the two shapes share no edge on purpose. Structure,
  // not verdict: it says "these two no longer coincide", never "this is wrong".
  stale: [
    ['M', 2.5, 2.5, 'L', 10.5, 2.5, 'L', 10.5, 10.5, 'L', 2.5, 10.5, 'Z'],
    ['M', 13.5, 8.5, 'L', 13.5, 13.5, 'L', 8.5, 13.5],
  ],
});

export const MARK_NAMES = Object.freeze(Object.keys(MARKS));

const pathText = (stroke) => stroke.join(' ');

/**
 * @param {string} name
 * @param {number} size in px; there is no default, because the default is how a mark
 *   becomes 300 px wide without anyone deciding it should be
 */
export function mark(name, size) {
  if (!Number.isFinite(size) || size <= 0) {
    throw new RangeError(`mark("${name}") needs a size in px; an unsized mark takes the format's default box`);
  }
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('width', String(size));
  svg.setAttribute('height', String(size));
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('focusable', 'false');
  svg.classList.add('mark');
  // Which mark this is, on the mark. A redraw that has to decide whether the mark it is
  // looking at is already the one wanted cannot ask an <svg> what it drew, and comparing
  // path data to find out would be reading the picture to recover the name.
  svg.dataset.mark = name;

  const strokes = MARKS[name];
  if (!strokes) {
    // Said, not swallowed. A mark nobody drew has to be visible, or the screen quietly
    // loses an affordance and looks correct while doing it.
    svg.classList.add('mark-unnamed');
    const box = document.createElementNS(NS, 'rect');
    box.setAttribute('x', '2.5');
    box.setAttribute('y', '2.5');
    box.setAttribute('width', '11');
    box.setAttribute('height', '11');
    svg.append(box);
    svg.setAttribute('data-unnamed', name);
    return svg;
  }
  for (const stroke of strokes) {
    const path = document.createElementNS(NS, 'path');
    path.setAttribute('d', pathText(stroke));
    svg.append(path);
  }
  return svg;
}
