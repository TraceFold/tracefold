// SPDX-License-Identifier: Apache-2.0
// The catalogue. Every reading in the harness is registered here, with the tier it
// is answerable at and the acceptance criteria it stands behind, both declared and
// neither inferred.
//
// This file does not touch the filesystem. Populations are selected out of the
// manifest the rig hands in, which is what makes "the denominator moved between two
// runs" unavailable rather than merely discouraged. A T0 reading enforces that.

import { createCatalogue } from './rig/catalogue.mjs';
import {
  hasSpdx, waitsOnCondition, takesPopulationFromRig, vocabularyIsNew, declaresNoDependency, ledgerParses,
} from './tiers/lint.mjs';
import { evaluates } from './tiers/load.mjs';
import { mounts, declaredSetMatches } from './tiers/mount.mjs';
import { inkBoxOverlap, usedGlyphSize, baselineInk } from './tiers/pixel.mjs';

// Modules that are safe to evaluate: importing a probe would start a renderer, and a
// tier that starts renderers as a side effect of being counted is a tier nobody can
// afford to run. Declared, so the gap is visible rather than assumed.
const EVALUABLE = (path) => path.endsWith('.mjs')
  && (path.startsWith('tools/rig/') || path.startsWith('tools/tiers/') || path.startsWith('membrane/src/'));
const ASSAY_SOURCE = (path) => path === 'tools/assays.mjs' || (path.startsWith('tools/tiers/') && path.endsWith('.mjs'));
const HARNESS_SOURCE = (path) => path.startsWith('tools/') && path.endsWith('.mjs');

export function buildCatalogue() {
  const catalogue = createCatalogue();

  // ---------------------------------------------------------------- T0
  catalogue.register({
    id: 'L-SPDX',
    tier: 'T0',
    title: 'every module in the harness carries a licence identifier',
    backs: ['AC-I39'],
    population: (world) => world.manifest.withExtension('.mjs'),
    hold: hasSpdx,
  });

  catalogue.register({
    id: 'L-DEPENDENCY',
    tier: 'T0',
    title: 'the harness declares no dependency',
    backs: ['AC-I10'],
    population: (world) => world.manifest.files.filter((f) => f.path.endsWith('package.json')),
    hold: declaresNoDependency,
    expect_empty: {
      reason: 'the harness has no manifest of its own because it has nothing to declare; the day one appears this reading starts counting it',
      expires: '2027-01-01',
    },
  });

  catalogue.register({
    id: 'L-DISK-REACH',
    tier: 'T0',
    title: 'no reading builds its own population',
    backs: ['AC-I1'],
    population: (world) => world.manifest.files.filter((f) => ASSAY_SOURCE(f.path)),
    hold: takesPopulationFromRig,
  });

  catalogue.register({
    id: 'L-CLOCK-WAIT',
    tier: 'T0',
    title: 'the harness waits on conditions and never on durations',
    backs: ['AC-I12'],
    population: (world) => world.manifest.files.filter((f) => HARNESS_SOURCE(f.path)),
    hold: waitsOnCondition,
  });

  catalogue.register({
    id: 'L-LEDGER-JSON',
    tier: 'T0',
    title: 'every hand-maintained ledger under tools/ parses as JSON',
    backs: ['AC-I40'],
    population: (world) => world.manifest.files.filter((f) => f.path.startsWith('tools/') && f.path.endsWith('.json')),
    hold: ledgerParses,
  });

  catalogue.register({
    id: 'L-VOCABULARY',
    tier: 'T0',
    title: 'the retired vocabulary was not carried across',
    backs: ['AC-I37'],
    population: (world) => world.manifest.files.filter((f) => HARNESS_SOURCE(f.path)),
    hold: vocabularyIsNew,
  });

  // ---------------------------------------------------------------- T1
  catalogue.register({
    id: 'LD-EVALUATES',
    tier: 'T1',
    title: 'every harness module imports and evaluates',
    backs: ['AC-I3'],
    armed_by: ['TB-LOAD'],
    population: (world) => world.manifest.files.filter((f) => EVALUABLE(f.path)),
    hold: evaluates,
  });

  // ---------------------------------------------------------------- T2
  catalogue.register({
    id: 'MT-FACE',
    tier: 'T2',
    title: 'every declared face mounts and produces the elements it declares',
    backs: ['AC-I25'],
    armed_by: ['TB-MOUNT'],
    requires: ['renderer'],
    population: (world) => world.faces,
    hold: mounts,
  });

  catalogue.register({
    id: 'MT-DECLARED-SET',
    tier: 'T2',
    title: 'the set that mounts and the set that is declared are the same set',
    backs: ['AC-I26'],
    requires: ['renderer'],
    population: (world) => {
      const declared = world.faces.map((f) => f.id);
      const present = world.faces.filter((f) => world.manifest.at(`tools/${f.source}`)).map((f) => f.id);
      return [{
        id: 'declared-vs-present',
        declaredOnly: declared.filter((id) => !present.includes(id)),
        observedOnly: present.filter((id) => !declared.includes(id)),
      }];
    },
    hold: declaredSetMatches,
  });

  // ---------------------------------------------------------------- T3
  catalogue.register({
    id: 'PX-INK-OVERLAP',
    tier: 'T3',
    title: 'no two text runs share painted area',
    backs: ['AC-I20', 'AC-I23'],
    requires: ['renderer'],
    armed_by: ['RT-07', 'RT-08b', 'TB-PIXEL'],
    population: (world) => world.faces.filter((f) => f.baseline),
    hold: async (face, world) => {
      const page = await world.openFace(face);
      try {
        const reading = await inkBoxOverlap(page);
        return reading.ok ? true : reading.message;
      } finally { await page.close().catch(() => {}); }
    },
  });

  catalogue.register({
    id: 'PX-GLYPH-SIZE',
    tier: 'T3',
    title: 'every glyph was used at the size it declares',
    backs: ['AC-I21'],
    requires: ['renderer'],
    armed_by: ['RT-08a'],
    population: (world) => world.faces.filter((f) => f.baseline),
    hold: async (face, world) => {
      const page = await world.openFace(face);
      try {
        const reading = await usedGlyphSize(page);
        return reading.ok ? true : reading.message;
      } finally { await page.close().catch(() => {}); }
    },
  });

  catalogue.register({
    id: 'PX-BASELINE',
    tier: 'T3',
    title: 'painted ink matches the committed baseline',
    backs: ['AC-I22', 'AC-I24'],
    requires: ['renderer', 'baselines'],
    armed_by: ['RT-07', 'RT-08a', 'RT-08b', 'TB-PIXEL'],
    population: (world) => world.faces.filter((f) => f.baseline),
    hold: async (face, world) => {
      const page = await world.openFace(face);
      try {
        const reading = await baselineInk(page, { baseline: world.baselineFor(face.id) });
        return reading.ok ? true : reading.message;
      } finally { await page.close().catch(() => {}); }
    },
  });

  return catalogue;
}
