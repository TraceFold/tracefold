// SPDX-License-Identifier: Apache-2.0
// T2 -- the thing is put on a real DOM and the result is read back.
//
// The failure this tier is answerable for: a reading that opens a face, finds an
// empty host, and reports that nothing was wrong with it. So the first question here
// is never "did it break", it is "did anything appear at all", and an empty host is
// a failure with its own sentence rather than a quiet pass.

export const MOUNT_MESSAGES = {
  MOUNTED: 'mounted and produced elements',
  HOST_EMPTY: 'mounted into a host that stayed empty, which is not a pass',
  TOO_FEW: 'mounted but produced fewer elements than it declares',
  GLYPHS_SHORT: 'mounted but produced fewer glyphs than it declares',
  UNREACHABLE: 'the face could not be opened',
};

const CENSUS = `(() => ({
  elements: document.body ? document.body.querySelectorAll('*').length : 0,
  glyphs: document.querySelectorAll('[data-glyph]').length,
  textLength: document.body ? document.body.innerText.replace(/\\s+/g, '').length : 0,
}))()`;

export async function mounts(face, world) {
  const renderer = world.renderer;
  if (!renderer) return MOUNT_MESSAGES.UNREACHABLE;
  let page;
  try {
    page = await renderer.openPage();
    await page.open(world.faceUrl(face));
    const census = await page.evaluate(CENSUS);
    if (census.elements === 0 || census.textLength === 0) return `${MOUNT_MESSAGES.HOST_EMPTY}: ${face.id}`;
    if (census.elements < face.expects.minElements) {
      return `${MOUNT_MESSAGES.TOO_FEW}: ${face.id} declares at least ${face.expects.minElements}, produced ${census.elements}`;
    }
    if (census.glyphs !== face.expects.glyphs) {
      return `${MOUNT_MESSAGES.GLYPHS_SHORT}: ${face.id} declares ${face.expects.glyphs} glyphs, produced ${census.glyphs}`;
    }
    return true;
  } catch (err) {
    return `${MOUNT_MESSAGES.UNREACHABLE}: ${face.id} -- ${err.message}`;
  } finally {
    if (page) await page.close().catch(() => {});
  }
}

// The denominator this tier runs on is the declared set, never the observed one.
// Both directions are failures: a face that was declared and did not appear, and a
// face that appeared and was never declared.
export function declaredSetMatches(entry) {
  if (entry.declaredOnly.length > 0) return `declared and not mountable: ${entry.declaredOnly.join(', ')}`;
  if (entry.observedOnly.length > 0) return `mountable and not declared: ${entry.observedOnly.join(', ')}`;
  return true;
}
