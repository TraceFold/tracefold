// SPDX-License-Identifier: Apache-2.0
// T0 -- readings whose only evidence is text.
//
// They are cheap, they are worth having, and they are carried on their own line
// because a text match is not evidence that anything ran. Nothing here reaches for
// the filesystem: every predicate takes an entry the rig already read.
//
// The patterns are not in this file. A rule spelled out inside the module that
// enforces it is a rule that finds itself -- the retired instrument lost two
// incidents to that. They live in rig/lint-patterns.json and arrive through world.

export const LINT_MESSAGES = {
  SPDX_PRESENT: 'carries an SPDX identifier',
  SPDX_ABSENT: 'no SPDX identifier in the opening lines',
  NO_DEPENDENCY: 'declares no runtime or development dependency',
  DEPENDENCY_DECLARED: 'declares a dependency',
  NO_DISK_REACH: 'takes its population from the rig',
  DISK_REACH: 'reaches for the filesystem instead of taking the population the rig hands it',
  NO_CLOCK_WAIT: 'waits on a condition',
  CLOCK_WAIT: 'waits on a duration, which ends whether or not the thing happened',
  VOCABULARY_CLEAR: 'uses none of the retired vocabulary',
  VOCABULARY_CARRIED: 'carries retired vocabulary',
  PATTERNS_ABSENT: 'the pattern catalogue was not handed to this reading',
};

const firstHit = (text, patterns) => {
  if (!Array.isArray(patterns)) return { error: LINT_MESSAGES.PATTERNS_ABSENT };
  for (const source of patterns) {
    if (new RegExp(source).test(text)) return { hit: source };
  }
  return {};
};

export const hasSpdx = (entry) => (entry.text.slice(0, 400).includes('SPDX-License-Identifier')
  ? true
  : `${LINT_MESSAGES.SPDX_ABSENT}: ${entry.path}`);

export const waitsOnCondition = (entry, world) => {
  const { hit, error } = firstHit(entry.text, world.patterns?.clockWait);
  if (error) return error;
  return hit ? `${LINT_MESSAGES.CLOCK_WAIT}: ${entry.path} matches ${hit}` : true;
};

export const takesPopulationFromRig = (entry, world) => {
  const { hit, error } = firstHit(entry.text, world.patterns?.diskReach);
  if (error) return error;
  return hit ? `${LINT_MESSAGES.DISK_REACH}: ${entry.path} matches ${hit}` : true;
};

export const vocabularyIsNew = (entry, world) => {
  const { hit, error } = firstHit(entry.text, world.patterns?.retiredVocabulary);
  if (error) return error;
  return hit ? `${LINT_MESSAGES.VOCABULARY_CARRIED}: ${entry.path} carries ${hit}` : true;
};

export const declaresNoDependency = (entry) => {
  let parsed;
  try { parsed = JSON.parse(entry.text); } catch { return `${entry.path} is not readable as json`; }
  const runtime = Object.keys(parsed.dependencies ?? {});
  const development = Object.keys(parsed.devDependencies ?? {});
  return runtime.length + development.length === 0
    ? true
    : `${LINT_MESSAGES.DEPENDENCY_DECLARED}: ${entry.path} -- ${[...runtime, ...development].join(', ')}`;
};

// AC-I40. tools/breaches.json sat malformed (missing comma) for an unknown length of
// time and nothing in this harness ever opened it, so the run kept printing a verdict
// with a broken ledger sitting behind it. The defect was not the comma -- it was that
// a hand-edited ledger with no reader can break silently. This reading is the reader.
export const LEDGER_JSON_MESSAGES = {
  PARSES: 'parses as JSON',
  MALFORMED: 'does not parse as JSON -- a hand-edited ledger with no reader can break silently',
};

export const ledgerParses = (entry) => {
  try {
    JSON.parse(entry.text);
    return true;
  } catch (error) {
    return `${LEDGER_JSON_MESSAGES.MALFORMED}: ${entry.path} -- ${error.message}`;
  }
};
