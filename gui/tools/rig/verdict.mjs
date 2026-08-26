// SPDX-License-Identifier: Apache-2.0
// The five words a reading is allowed to end in, and the seven ways a run is allowed
// to leave. Both are enumerations on purpose: an exit code is a state machine
// (INHERITED_PRINCIPLES §3e), and folding two states onto one number is how a tree
// that was never measured comes to look like a tree that was measured and was fine.
//
// SOURCES_ABSENT was added by req/883 for the seventh case, which is that sentence
// happening literally. A published tree does not ship req/, so the acceptance-criteria
// corpus this harness counts against is not there; coverage then read `0 / 0`, which a
// reader parses as "no gap" when it means "no data", and the run could still reach
// GREEN and citableAsEvidence:true. Reusing RED would have failed closed but lied in
// the other direction -- RED means measured and broken. This is neither verdict.

export const VERDICT = {
  PASS: 'PASS',
  FAIL: 'FAIL',
  EMPTY: 'EMPTY',
  SKIP: 'SKIP',
  FLAKY: 'FLAKY',
};

export const EXIT = {
  GREEN: 0,
  RED: 1,
  NON_CANONICAL: 2,
  PARTIAL: 3,
  FLAKY: 4,
  SELF_BLIND: 5,
  SOURCES_ABSENT: 6,
};

export const OUTCOME = {
  GREEN: 'GREEN',
  RED: 'RED',
  NON_CANONICAL: 'NON-CANONICAL',
  PARTIAL: 'PARTIAL',
  FLAKY: 'FLAKY',
  SELF_BLIND: 'SELF-BLIND',
  SOURCES_ABSENT: 'SOURCES-ABSENT',
};

// Every line this harness can print about a reading, in one place, in English, with
// the refusal and the acceptance both written down (5 principles, third).
export const RIG_MESSAGES = {
  TIER_MISSING: 'an assay was registered without declaring its tier',
  TIER_UNKNOWN: 'an assay declared a tier that is not in the ladder',
  ID_DUPLICATE: 'two assays claim the same id',
  BACKS_MISSING: 'an assay was registered without saying which acceptance criteria it stands behind',
  POPULATION_MISSING: 'an assay was registered without a population selector',
  EMPTY_UNEXPLAINED: 'the population was empty and no reason was recorded, so this is not a pass',
  EMPTY_EXPLAINED: 'the population was empty for a recorded reason that has not expired',
  EMPTY_EXPIRED: 'the population was empty and the recorded reason has expired',
  HELD: 'held over every member of the population',
  BROKE: 'did not hold',
  REQUIREMENT_ABSENT: 'a requirement of this reading is not present here, so it was not run',
  RESULT_ALREADY_RECORDED: 'a second result was offered for an id that already has one',
  TREE_MOVED: 'the tree changed while the run was in progress, so nothing here was measured against one tree',
  ASSAY_TOUCHED_DISK: 'an assay reached for the filesystem instead of the population the rig handed it',
  GREEN_WITHOUT_WIRE: 'a run that never reached a real server is not green',
  AC_SOURCES_ABSENT: 'the acceptance-criteria corpus is not in this tree, so coverage was not measured -- the number below is an empty denominator, not a clean one',
  RUN_GREEN: 'every reading held, over a population that was not empty',
};

export const TIERS = ['T0', 'T1', 'T2', 'T3', 'T4'];

// T0 is evidence about text. It is worth having and it is not worth counting as a
// reading of behaviour, so it is carried on its own line and never summed in.
export const COUNTED_TIERS = ['T1', 'T2', 'T3', 'T4'];
