// SPDX-License-Identifier: Apache-2.0
// KA-I3, in a time box: can a renderer be driven with node builtins alone?
// One round trip to Page.captureScreenshot is the whole question. If this does not
// land, req/06 §8-2 says drop to the fallback here rather than push on.
//
//   node tools/probes/cdp_reach.mjs

import { startRenderer, findRenderer } from '../rig/renderer.mjs';

const started = Date.now();
const binary = findRenderer();
console.log(`renderer binary : ${binary ?? '<none found>'}`);
if (!binary) { console.log('verdict         : UNREACHABLE (no binary)'); process.exit(1); }

let renderer;
try {
  renderer = await startRenderer({ viewport: { width: 400, height: 200 } });
  console.log(`endpoint        : ${renderer.endpoint}`);
  console.log(`product         : ${renderer.product}`);
  const page = await renderer.openPage();
  await page.open('data:text/html,<body style="margin:0;background:%23fff"><p id=t>reach</p></body>');
  const text = await page.evaluate("document.getElementById('t').textContent");
  const png = await page.capture();
  const isPng = png.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
  console.log(`evaluate        : ${JSON.stringify(text)}`);
  console.log(`screenshot      : ${png.length} bytes, png signature ${isPng ? 'ok' : 'MISSING'}`);
  console.log(`elapsed         : ${Date.now() - started} ms`);
  console.log(`verdict         : ${isPng && text === 'reach' ? 'REACHABLE (zero dependencies)' : 'UNREACHABLE'}`);
  await page.close();
  process.exitCode = isPng && text === 'reach' ? 0 : 1;
} catch (err) {
  console.log(`verdict         : UNREACHABLE -- ${err.message}`);
  process.exitCode = 1;
} finally {
  if (renderer) await renderer.stop();
}
