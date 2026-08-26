# tools/ — the instrument

Written from `req/06_INSTRUMENTS.md`. Zero dependencies, node builtins only. Nothing here is carried over from the retired instruments — not a line, not an identifier.

## Entry points

| command | what it answers |
|---|---|
| `node tools/verify-all.mjs [--repeat N] [--json <path>]` | the only command allowed to print a verdict about the whole tree |
| `node tools/verify-self.mjs` | does the harness notice being broken (runs first inside `verify-all`) |
| `node tools/baseline.mjs list \| commit <face> --reason "…" \| retire <face> --reason "…"` | the committed-ink ledger |

Exit codes are a state machine, not a boolean: `0` green, `1` red, `2` non-canonical (the tree moved under the run), `3` partial (something was not measured), `4` flaky, `5` self-blind (the harness failed its own rounds, so every other number in that run is void).

## Probes

These killed the assumptions the design rests on, before any reading was written. They are kept because a claim without the run that produced it is a claim.

| probe | question | measured |
|---|---|---|
| `probes/cdp_reach.mjs` | can a renderer be driven with builtins alone | reachable, 898 ms, first round trip |
| `probes/pixel_determinism.mjs` | is a capture of the same page the same capture | byte-identical across 5 captures, 5 loads and 3 renderer restarts |
| `probes/rt_red_first.mjs` | do the pixel readings go red on the defects they exist for | 12 readings, 12 agreed with the plan |
| `probes/tier_red_first.mjs` | do the load, mount and pixel readings turn under a live round | 3 rounds landed, nothing restored badly, no collateral |

## Ledgers

`faces.json` declared mount set · `breaches.json` live rounds · `meta-breaches.json` rounds fired at the harness itself · `baselines/LEDGER.json` committed ink · `rig/lint-patterns.json` the text patterns, kept out of the module that applies them so a rule cannot match itself.

## Not built yet

A breach runner (`req/06` stage I5) — the rounds in `breaches.json` are fired by the two probes by hand, so a row's result can go stale with nothing saying so. The wire tier (I9) — no server is stood up, `wire=no`, and no run can therefore be green. Determinism quarantine, evidence floor, exemption ledger and the copy gates (I4, I10, I11) are unwritten, which is why 26 of the 39 acceptance criteria print as unbacked.
