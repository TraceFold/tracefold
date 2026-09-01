# receipt-memory-export

Derives generic agent-memory nodes from `gx` receipts, using CLI output only.

## The claim

Memory is *downstream* of a signed receipt. Every node this exporter writes is
a pure function of bytes `gx` already printed to stdout for some CLI verb --
`gx plan`, `gx commit`/`gx undo`, `gx receipt show --level 3`. It never reads
`.gx/` state, never opens the journal or the ledger, and never imports any
`gx-*` crate. If the project this ran against disappeared, the harness's
`captures/` directory alone is enough to reproduce every node byte for byte --
so memory here is *derived*, hence regenerable, not a second place of record.

## Files

- `export_memory.py` -- the exporter. Reads a `captures/` directory (see its
  own module docstring for the exact input contract) and writes
  `memory/nodes/<node_id>/{node.json,node.md}` plus `memory/index.{json,md}`.
  `--openviking-style` additionally emits an addressable-path layout
  *inspired by* observing OpenViking's documented L0/L1/L2 staged-disclosure
  README and `viking://`-style URI (req/998 C-10/C-11) -- no OpenViking
  source exists in this checkout or was read to write this (COPY HARD BAN:
  observation of documented behaviour is fine, transcribing code is not).
  It is an approximation of a *shape*, not a claim of wire compatibility.
- `run_demo.sh` -- a copy-paste starting point (not wired into this repo's
  CI, same as `examples/demo_one_screen.sh` beside it) that drives a real
  `gx` project through `submit -> plan -> verify -> commit` three times and
  one `undo`, captures each verb's own JSON, runs the exporter over it, and
  then verifies the result against the live project.

## What the exporter does *not* have

There is no `gx --list-receipts` this script calls. There isn't one --
measured by reading `crates/gx-cli/src/main.rs`'s `Command` enum in full
(2026-09-01): `gx replay` reconstructs Sigma and reports a match/diff
summary, not a per-record list; `gx draft list` covers undrafted intents,
not committed ones; `gx verdict-checkpoint list` lists signed *counts*, not
receipts. So "list" here is the harness's own manifest of the transformation
ids it drove -- disclosed, not papered over.

## What one node covers, and what it deliberately excludes

One node = one **committed** transformation (a `gx commit` or a `gx undo`),
not one file under `.gx/receipts/`. `gx verify` also writes a receipt (a
`VerdictReceipt`) but that receipt attests a plan was admissible, not that
anything changed -- it never becomes a memory node. Measured on a real run:
3 commits + 1 undo produced 4 `*.commit.json` + 3 `*.verdict.json` = 7 files
on disk; `node_count` is 4, matching the harness's own manifest count, not
the raw file count. `run_demo.sh` checks and prints both numbers rather than
comparing against the raw total.

## Running it for real

Needs a built `gx` on `PATH`, or `GX=/path/to/gx` set first. `$HOME` has to
be on a real POSIX filesystem while it runs -- `docs/TUTORIAL.md` §2 measured
`gx key gen` refusing to *load* a key whose store is on a filesystem that
cannot hold Unix permissions (a Windows drive seen through WSL's `/mnt/c`, a
network share). The script does not override `$HOME`; run it from a shell
whose `$HOME` already satisfies that (WSL user home, not `/mnt/c/...`).

```sh
export GX=/path/to/target/debug/gx   # or put gx on PATH
bash run_demo.sh 1
```

It exits non-zero on any check failure and prints `RESULT=PASS`/`RESULT=FAIL`
on the last line.

## Verified (2026-09-01, real `gx 0.1.0` build, three separate runs)

- `node_count == source_manifest_transformation_count` (the exporter's own
  print) -- 4 == 4, all three runs.
- `node_count == receipt_files_commit_shaped` on the live project's
  `.gx/receipts/` -- 4 == 4, all three runs (the raw total including
  `*.verdict.json` was 7, disclosed separately, not treated as a mismatch).
- Round trip: every node's `receipt_reference.transformation` resolves back
  through `gx receipt show <TID> --level 1 --json` to the same id -- 4/4,
  all three runs.
- Supersede evidence: the node for the transformation that got undone shows
  `found_superseding_undo=true` naming the undo's id; the two untouched
  commits show `false`; the undo's own `reversibility_at_commit` is one of
  gx's three real C-25 answers (observed: `"true"` -- an inverse to the undo
  itself, i.e. redo, was constructed) -- all three runs.

An earlier draft of `run_demo.sh`'s self-check asserted a fixed value
(`'not_applicable_is_itself_an_undo'`) for the undone-undo's own
`reversibility_at_commit` that `export_memory.py` never emits -- written
before this script had ever been run against a real `gx` build. Fixed to
assert the field is one of the three real values and print which one was
observed, instead of guessing a fourth. Same pass also fixed a literal
backtick inside a `python3 -c "..."` double-quoted block in the same script
(`` `gx 0.1.0` ``) that bash was expanding as a command substitution --
harmless here (it only polluted stderr with `gx: command not found`) but a
real bug, not a style nit.

## Not covered by this exporter (disclosed, not silently narrowed)

- Only `fs`-substrate transformations were exercised by `run_demo.sh`; other
  substrates are not exhaust-tested here.
- `supersede_evidence` is scoped to *this export's own capture batch*, not
  the project's full receipt history -- a `not_found`/`false` result means
  "no undo naming this transformation is in this batch," never "this
  transformation is still undoable."
- The `--openviking-style` output is an observed-shape approximation, not a
  wire-compatible implementation of anything OpenViking ships.
