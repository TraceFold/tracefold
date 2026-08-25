# gx-substrate-conformance

The adapter-independent contract harness 51 §7 asks for: seven contracts (1:1 with 51 §7's own
table) plus the laws the rulings added (`req/69` §3.4's L-list, `K1`/`K2`). Every `SubstrateAdapter`
implementation in this workspace (`gx-adapter-fs`, `gx-adapter-git`, `gx-adapter-mcp`,
`gx-adapter-postgres`) is required to pass all sixteen obligations before it is considered complete
(`Report::meets_51_7`).

This file is the **third-party entry point** (`req/506_CONFORMANCE_REQDEF_2026-08-22.md` P0): how
to run the same suite each adapter's own author runs, from outside this repository's implementation
history, and how to read what comes back. It packages what already exists -- `contracts.rs` and
`laws.rs` are unmodified, and no new obligation is added here.

## Run it

From WSL2 Ubuntu-24.04 (cargo is blocked on the Windows side by Smart App Control, `req/05` §5):

```bash
bash tools/conformance_adapters.sh
```

This runs the fs, git and mcp adapters' own `#[test]` (each inherits the harness by calling
`gx_substrate_conformance::run_all` once, `src/lib.rs`'s documented shape) and prints one summary
line per adapter plus a table. All three are expected to be **16/16 green** (7 contracts + 9 laws)
with no live external service required.

The postgres adapter needs a real server, so it is handled differently on purpose (see below).

### Reading the output

Each adapter's own test prints a line shaped like:

```
CONFORMANCE <adapter>: CHECKS=16 PASS=16 FAIL=0 NOT_SUPPLIED=0 CONTRACT=7 LAW=9 conformant=true complete=true
```

Three questions, kept separate (**§31 M4H3-4 (b)**, `src/lib.rs`'s `Report`):

- `conformant` -- zero **failures**. Nothing measured contradicted 51 §7 or a ruling.
- `complete` -- zero **unmeasured**. Every obligation had a subject to run against (a partially
  built adapter that has not implemented a method yet is "NOT_SUPPLIED", never a silent pass and
  never a failure).
- `meets_51_7` -- both of the above. 51 §7's own completion condition: "no adapter satisfies the
  M4/M7 completion condition unless it passes all seven of the above contracts".

`tools/conformance_adapters.sh` reduces this further, per adapter, to one of three row-states:
**GREEN** (`meets_51_7` / the test's own assertions held), **FAIL** (a real defect -- something
this adapter promises does not hold), or **NOT_RUN** (postgres only, see next section -- an
environment the operator has not set up yet, never conflated with FAIL).

## The postgres leg, specifically

`crates/gx-adapter-postgres/tests/pg_conformance.rs` needs a live server reachable through
`GX_ADAPTER_POSTGRES_DSN_DEFAULT` (`tools/pg_local.sh env` prints the line once the server is up).
Without it, the fixture's own connection helper panics with a named "DSN must be set" message --
which, read cold, looks exactly like a code defect to anyone who has not read this file.

`tools/conformance_adapters.sh` checks for the environment variable **before** touching cargo:

- **unset** -> a `NOT_RUN` row. Nothing about the adapter's code is asserted either way; this is
  reported as an environment gap, not folded into "conformant" and not folded into "failed".
- **set** -> the postgres leg runs for real, held to the identical 16/16 bar as fs/git/mcp
  (`req/115` §A-3: "the same-shaped denominator as adapter-git's 16/16, fixed at connection time").

To exercise the postgres leg for real:

```bash
bash tools/pg_local.sh setup   # once: extracts a real, unmodified PostgreSQL 16 (no root needed)
bash tools/pg_local.sh start
eval "$(bash tools/pg_local.sh env)"
bash tools/conformance_adapters.sh
```

`tools/pg_local.sh` runs an unprivileged, real Postgres 16.14 server under `$HOME`, reachable only
from the account that started it -- not docker, not a mock (see the script's own header for why).

## Negative control

`crates/gx-substrate-conformance/tests/broken_fixture.rs` is the harness's own standing negative
control: eighteen deliberate flaws, one obligation broken at a time, asserting that each is
reported as `Fail` (or, for the one flaw that means "not implemented yet", `NotSupplied` --
**never** a silent pass). Include it in a run with:

```bash
bash tools/conformance_adapters.sh negctrl
```

This is what stands behind the claim that the packaging above is not decorative: an entry point
that only ever prints GREEN would be indistinguishable from one that ignores its own test results.
`broken_fixture.rs`'s eighteen flaws, run through the same `run_leg` path as the four adapters, are
the proof that a real defect turns this script's exit code and summary row red -- P0's own
verification exercised this directly by flipping one guard the same way `req/76` §2.2's cited
mutant does (`laws.rs`'s L5 comparison, `applied.resulting_digest() == &target` -> unconditionally
`true`) and confirming `tools/conformance_adapters.sh negctrl` reports `FAIL` for exactly that
obligation before the guard was reverted; see `req/511_CONFORMANCE_P0_REPORT_2026-08-22.md` for the
transcript. No such mutation is left in the tree -- this is a repeatable verification step, not a
fixture that ships broken.

## What this is not

- Not `tools/conformance_gen.sh` / `tools/conformance_smoke.sh` -- those drive M8's Lean canon-model
  differential-vector suite (`crates/gx-canon`, `crates/gx-gate`, `crates/gx-witness`'s
  `*_conformance_gen` tests + `lake exe runner`). One word apart, one milestone apart, no shared
  code (**N-12**, `src/lib.rs` documents the same distinction from the crate's own side).
- Not a claim about receipt/wire-format conformance (docs/LIMITS.md's hermetic-verification claim,
  the CRUD capability catalogue). Those are `req/506` P1/P2, out of this P0's scope, and will get
  their own entry points under `crates/gx-witness/tests/`, `crates/gx-cli/tests/` and
  `crates/gx-api/tests/` respectively.
- Not a new crate, a new binary or a new `SubstrateAdapter` obligation. `contracts.rs` and
  `laws.rs` are unmodified; this README and `tools/conformance_adapters.sh` are the entirety of
  P0's write surface (`req/506` §3).
