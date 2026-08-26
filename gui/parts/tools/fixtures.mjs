// SPDX-License-Identifier: Apache-2.0
// The smallest page that puts each drawing part in front of a renderer.
//
// The fixtures are generated from the parts rather than written beside them, so what
// gets photographed is the tree the unit tests read. A hand-written fixture is a
// second implementation, and a second implementation is how a suite ends up green
// about something that never shipped.
//
// Width matters here. The window this application ships in is 720 across, which is
// below the breakpoint that collapses --detail-x to zero, so the collapsed case is
// the ordinary case rather than an edge one (req/04a C1, defect 2). The default
// capture width is therefore the narrow one: the hostile width is the normal width.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { el, style, toHtml } from '../src/element.mjs';
import { CONSUMED, readTokenSource, contrastTable, AA_NORMAL_TEXT } from '../src/tokens.mjs';
// Node-only: resolves a real path against a real disk. tokens.mjs no longer carries
// this (req/02 W15 -- a browser loads tokens.mjs and has no node:path to resolve).
import { tokenHref } from './token-source.mjs';
import { sheet, glyph, everyMark, RED_RULE } from '../src/glyph-sheet.mjs';
import { badge } from '../src/verdict-badge.mjs';
import { receiptRow } from '../src/receipt-row.mjs';
import { fold } from '../src/provenance-fold.mjs';
import { serial } from '../src/serial.mjs';
import { claimOf } from '../src/seal-claim.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
export const FIXTURE_DIR = path.resolve(HERE, '../fixtures');
export const SHOT_DIR = path.join(FIXTURE_DIR, 'shots');

export const NARROW = { width: 720, height: 1000 };
export const WIDE = { width: 1280, height: 700 };

const heading = (words) => el('h2', {
  style: style({
    margin: '0 0 8px', color: CONSUMED.attendant, 'font-family': CONSUMED.sans,
    'font-size': CONSUMED.meta, 'line-height': CONSUMED.metaLine, 'font-weight': '600',
    'letter-spacing': '0.08em', 'text-transform': 'uppercase',
  }),
}, [words]);

const block = (words, children) => el('section', {
  style: style({ padding: '16px 0', 'border-bottom': `1px solid ${CONSUMED.rule}` }),
}, [heading(words), ...children]);

function page({ title, body }) {
  const href = tokenHref(FIXTURE_DIR);
  const shell = el('main', {
    style: style({ padding: '0 16px 40px', 'max-width': '100%' }),
  }, body);
  return [
    '<!doctype html>',
    '<html lang="en">',
    '<head>',
    '<meta charset="utf-8">',
    `<title>${title}</title>`,
    `<link rel="stylesheet" href="${href}">`,
    '<style>',
    'html,body{margin:0;padding:0}',
    `body{background:${CONSUMED.page};color:${CONSUMED.ink};font-family:${CONSUMED.sans};font-size:${CONSUMED.record};line-height:${CONSUMED.recordLine}}`,
    '</style>',
    '</head>',
    '<body>',
    toHtml(sheet()),
    toHtml(shell),
    '</body>',
    '</html>',
    '',
  ].join('\n');
}

const RECORDS = [
  { id: 'r-01', n: 1, at: '09:14:02', actor: 'agent/packer', effect: 'wrote', path: 'src/index.mjs', verdict: 'Admit', digest: 'a73e393d8a97b86c', basis: 'exact', algorithm: 'blake3', anchor: 'tile/44' },
  { id: 'r-02', n: 2, at: '09:14:07', actor: 'agent/packer', effect: 'removed', path: 'build/cache/very/long/path/that/will/not/fit/in/the/column/at/720/across/artifact.bin', verdict: 'Deny', digest: 'b1c2d3e4f5a6b7c8', basis: 'derived' },
  { id: 'r-03', n: 3, at: '09:14:19', actor: 'human/owner', effect: 'approved', path: 'policy/escalation.toml', verdict: 'Escalate', digest: 'ffee0011aabb2233', basis: 'exact' },
  { id: 'r-04', n: 4, at: '09:15:00', actor: 'agent/packer', effect: 'wrote', path: 'src/index.mjs', verdict: 'Admit', digest: '00112233445566ab', holes: { actor: 'the calling identity was not recorded by the issuer' } },
  { id: 'r-05', n: 5, at: '09:15:31', actor: 'human/owner', effect: 'undone', path: 'src/index.mjs', verdict: 'Admit', childOf: 'r-01', digest: 'cafebabe12345678', basis: 'exact' },
];

const LONG_NOTE = [
  { name: 'why', value: 'the packer asked to remove a build artifact that the standing rule holds for review, and the rule was applied by this window rather than by the engine, which is the part of this sentence that matters and the part that used to be drawn on top of the row beneath it' },
  { name: 'rule', value: 'no removal under build/ without a named owner present in the same session' },
  { name: 'applied by', value: 'this window, not independently' },
];

export function fixtures() {
  const verifier = { name: 'gx-verify', independent: false };

  const marks = everyMark();
  const tokensPage = () => {
    const table = contrastTable(readTokenSource());
    return page({
      title: 'tokens',
      body: [
        block('text at each ink this package consumes', [
          el('p', { style: style({ margin: '0 0 4px', color: CONSUMED.ink }) }, ['subject ink: the value a person came to read']),
          el('p', { style: style({ margin: '0 0 4px', color: CONSUMED.attendant }) }, ['attendant ink: when it happened, where it came from, what it is not']),
          el('p', { style: style({ margin: '0', color: CONSUMED.ink, 'font-family': CONSUMED.mono, 'font-size': CONSUMED.time }) }, ['mono is for what was typed or hashed: 09:14:02  a73e393d8a97b86c']),
        ]),
        block('measured against its own page, computed from the stylesheet of record', [
          el('div', {}, table.map((r) => el('div', {
            style: style({ display: 'grid', 'grid-template-columns': '4rem 6rem 6rem 1fr', gap: '10px', padding: '2px 0', 'font-family': CONSUMED.mono, 'font-size': CONSUMED.meta }),
          }, [
            el('span', { style: style({ color: CONSUMED.attendant }) }, [r.side]),
            el('span', { style: style({ color: CONSUMED.ink }) }, [r.name]),
            el('span', { style: style({ color: CONSUMED.ink }) }, [`${r.ratio.toFixed(2)}:1`]),
            el('span', { style: style({ color: CONSUMED.attendant }) }, [r.ratio >= AA_NORMAL_TEXT ? 'clears the normal-text floor' : 'below the normal-text floor: not used for text here']),
          ]))),
        ]),
      ],
    });
  };

  const glyphPage = () => page({
    title: 'glyph-sheet',
    body: [
      block('every mark, at three stated sizes', [
        el('div', {}, marks.map((mark) => el('div', {
          style: style({ display: 'grid', 'grid-template-columns': '10rem 24px 24px 24px 1fr', gap: '12px', 'align-items': 'center', padding: '4px 0' }),
        }, [
          el('span', { style: style({ color: CONSUMED.ink, 'font-family': CONSUMED.mono, 'font-size': CONSUMED.meta }) }, [`${mark.namespace}/${mark.key}`]),
          glyph(mark.namespace, mark.key, { size: 14 }),
          glyph(mark.namespace, mark.key, { size: 18 }),
          glyph(mark.namespace, mark.key, { size: 22 }),
          el('span', { style: style({ color: CONSUMED.attendant, 'font-size': CONSUMED.meta }) }, [mark.means]),
        ]))),
      ]),
      block('a name this sheet does not hold', [
        el('div', { style: style({ display: 'flex', 'align-items': 'center', gap: '12px' }) }, [
          glyph('standing', 'invented-word', { size: 22 }),
          el('span', { style: style({ color: CONSUMED.attendant, 'font-size': CONSUMED.meta }) }, ['drawn and labelled, not left blank']),
        ]),
      ]),
      block('the rule the sheet carries', [
        el('p', { style: style({ margin: '0', color: CONSUMED.attendant, 'font-size': CONSUMED.meta }) }, [RED_RULE]),
      ]),
    ],
  });

  const badgePage = () => page({
    title: 'verdict-badge',
    body: [
      block('the three words the engine says, and one it does not', [
        el('div', { style: style({ display: 'flex', 'flex-direction': 'column', gap: '10px', 'align-items': 'flex-start' }) }, [
          badge('Admit'), badge('Deny'), badge('Escalate'), badge('approved'), badge(null),
        ]),
      ]),
      block('the same badges with the word withheld, so only the shape carries it', [
        el('div', { style: style({ display: 'flex', gap: '20px', 'align-items': 'center' }) }, [
          badge('Admit', { size: 22, showWord: false }),
          badge('Deny', { size: 22, showWord: false }),
          badge('Escalate', { size: 22, showWord: false }),
        ]),
      ]),
    ],
  });

  const rowPage = () => page({
    title: 'receipt-row',
    body: [
      block('rows at one pitch, with a note open under the second', [
        el('div', {}, RECORDS.map((record) => receiptRow(record, {
          claim: claimOf(record, { verifier: record.basis === 'exact' ? verifier : null }),
          note: record.id === 'r-02' ? LONG_NOTE : null,
          open: record.id === 'r-02',
          }))),
      ]),
      block('a row with no claim passed, and a row with a declared hole', [
        el('div', {}, [
          receiptRow({ id: 'r-06', n: 6, at: '09:16:04', actor: 'agent/packer', effect: 'wrote', path: 'docs/LIMITS.md', verdict: 'Admit' }, {}),
          receiptRow({ id: 'r-07', n: 7, at: '09:16:40', effect: 'wrote', path: 'docs/LIMITS.md', verdict: 'Deny', holes: { actor: 'the issuer did not record who asked', verdict: 'the engine answered after the window stopped listening' } }, { claim: claimOf({}, {}) }),
        ]),
      ]),
    ],
  });

  const foldPage = () => page({
    title: 'provenance-fold',
    body: [
      block('the claims, which do not live in the fold', [
        el('dl', { id: 'claims', style: style({ margin: '0' }) }, [
          el('dt', { id: 'serial', style: style({ color: CONSUMED.attendant, 'font-size': CONSUMED.meta }) }, ['serial']),
          el('dd', { style: style({ margin: '0 0 8px', 'font-family': CONSUMED.mono }) }, ['A73E39']),
          el('dt', { id: 'false-when', style: style({ color: CONSUMED.attendant, 'font-size': CONSUMED.meta }) }, ['false when']),
          el('dd', { style: style({ margin: '0' }) }, ['the recomputed digest differs from the one recorded here']),
        ]),
      ]),
      block('both halves, one of them empty', [
        fold({
          summary: 'where that came from, and who applied it',
          open: true,
          settled: [
            { name: 'falsifier origin', value: 'a standing rule, not one written for this record' },
            { name: 'checked by', value: 'this window, which is not independent of the issuer' },
          ],
          held: [],
        }),
      ]),
      block('both halves carrying entries', [
        fold({
          summary: 'where that came from, and who applied it',
          open: true,
          settled: [{ name: 'anchor', value: 'tile/44, written 09:12:00' }],
          held: [{ name: 'awaiting', value: 'an owner present in this session' }],
        }),
      ]),
    ],
  });

  const serialPage = () => page({
    title: 'serial',
    body: [
      block('a sixteen character digest', [serial('a73e393d8a97b86c')]),
      block('a 64 character digest, described by the same code and the same sentence', [serial('a73e393d8a97b86c1122334455667788990011223344556677889900aabbccdd')]),
      block('a digest that cannot be cut', [serial('not-a-digest')]),
    ],
  });

  return [
    { name: 'tokens.html', html: tokensPage(), viewports: ['narrow'] },
    { name: 'glyph-sheet.html', html: glyphPage(), viewports: ['narrow', 'dark'] },
    { name: 'verdict-badge.html', html: badgePage(), viewports: ['narrow'] },
    { name: 'receipt-row.html', html: rowPage(), viewports: ['narrow', 'wide', 'dark'] },
    { name: 'provenance-fold.html', html: foldPage(), viewports: ['narrow'] },
    { name: 'serial.html', html: serialPage(), viewports: ['narrow'] },
  ];
}

export function writeFixtures() {
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  const written = [];
  for (const fixture of fixtures()) {
    const target = path.join(FIXTURE_DIR, fixture.name);
    fs.writeFileSync(target, fixture.html, 'utf8');
    written.push({ ...fixture, path: target, bytes: Buffer.byteLength(fixture.html) });
  }
  return written;
}

// Entry decided by the module's own url against the argument node was started with,
// never by the file's name (a name can be shared; this cannot).
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const f of writeFixtures()) process.stdout.write(`${f.name} ${f.bytes} bytes\n`);
}
