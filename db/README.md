# db — the TraceFold semantic log compiler

`db` reads a corpus of markdown, cuts it into atoms that keep a byte-exact address back into the
source, records what it admitted in an append-only journal, and answers questions about the result
through a SQLite index it can throw away and rebuild. It exists to test one assumption: that asking
a compiled corpus returns fewer bytes than reading and grepping the files.

The corpus is four layers, and nothing is allowed to do two of their jobs.

| layer | what it holds | who writes it | how it changes |
|---|---|---|---|
| **Source** | `db.toml`, `bands/<band>/band.toml`, the `*.md` themselves, and `journal/` | people and lanes; the journal only by `db push` | the markdown freely, the journal by appending |
| **Semantic IR** | band → document → section → atom | `db compile`, and nothing else | derived; editing it by hand is meaningless |
| **Index** | SQLite tables plus an FTS5 index | `db compile` | delete it and compile again; the digests come back the same |
| **Query** | `ls`, `show`, `find`, over stdout or loopback HTTP | read only | it never writes |

This folder is the engine. It holds no data. Everything it produces is written under a `DB/`
directory somewhere else — `bands/` is the writing, `journal/` is what was admitted and when, and
`build/` is everything that can be deleted without losing anything.

## Layout

```
Cargo.toml                    workspace, one member
crates/db/
  src/atom.rs                 the atom, its content id, and the three-valued fields
  src/manifest.rs             db.toml and band.toml; refuses rather than defaulting
  src/extract.rs              markdown into atoms, every byte claimed by exactly one
  src/store.rs                journal and hash chain, the sqlite index, the raw tier
  src/route.rs                filters, caps, cursors, FTS5 search, and the json wire
  src/gate.rs                 every gate, each carrying its own controls
  src/serve.rs                loopback http, GET only, same json as stdout
  src/main.rs                 the command line
  tests/controls.rs           the controls, run against the built binary
  tests/wire_schema.rs        asserts the engine's own json against schema/wire.json
schema/wire.json              the one shape every answer takes; the source both sides read
tools/readme_sync.mjs         derives two sections of this file from the command tree
tools/bench.mjs               times every command two ways and writes build/bench.jsonl
tools/wire_schema.mjs         generates face/src/wire.generated.ts from schema/wire.json
tools/publish_floor_gate.mjs  recounts tests/ against the numbers the Tests section declares
face/src/wire.generated.ts    the generated typescript a face imports; edit the schema, not this
```

One thing that sits beside this crate in the tree it was developed in is not in this folder:
`superseded/`, two retired files replaced by `gate.rs` and `db.toml`, recoverable from the
private tree's own git history rather than carried here. Everything else pictured above —
including the two scripts the sections below are marked as derived from — is in this folder.

## Tests

`tests/` is carried whole, not summarised: one file of controls and the fixture corpora they
build against. Nothing under it was picked out, cut down or paraphrased for this folder.

| what | count |
|---|---|
| files under `tests/` in this folder | 10 |
| files left out when this folder was assembled | 0 |
| control functions in `tests/controls.rs` | 48 |
| test functions in `tests/wire_schema.rs` | 3 |

Every fixture is invented text — nobody's directory tree, no real corpus, no request that ever
answered a browser. That was checked, not assumed: two independent scanners (`grep -rInE` and a
PowerShell `Select-String` in regex mode, because `-SimpleMatch` silently returns zero on these
patterns) searched `tests/` for a filesystem path outside this repository, a credential-shaped
string and an email address, each scanner first proven alive against a planted file that must
match. Both returned zero on all three patterns; `tools/publish_floor_gate.mjs` reruns the
file count and refuses if it stops matching the numbers above.

The previous count here (21 controls, and 9 files rather than 10) was `src/` and `tests/` out of
step: the engine this crate is developed against had already grown `gate --json`, the `/v1/gate`
route, `--include-gaps` and the commands and fields this sync brought over, and the seven extra
controls that exercised them were carried into a `tests/` that could not yet build against them.
That gap is closed here — this sync moved `src/` and `tests/` together rather than either alone —
and it was checked, not assumed: `crates/db/` was built from nothing on a fresh target directory
inside WSL and `cargo test` run before anything else in this section was written down. 48 plus 3
passed, 0 failed.

## Commands

Derived from the command tree by `tools/readme_sync.mjs`; edit the `#[command]` and `#[arg]
help text in `crates/db/src/main.rs`, never this section.

```
db init <DIR>              create a DB at a directory: db.toml, one band, an empty journal and an empty build/, refusing rather than overwriting anything already there
db compile                 read the source, build the semantic index, print the counts and the digests
db push                    record the atoms that changed as admission events in the journal, then compile
db gate [flags]            run every source, index and query gate over this DB and print pass, fail and UNKNOWN
db bands [flags]           list the bands this DB declares, with the document and atom count of each
db ls [flags]              list the atoms of a projection
db show <ADDRESS> [flags]  print one atom named by its id or by its exact address
db find <NEEDLE> [flags]   search the full text index and return addresses and scores, never bodies
db serve [flags]           answer the same wire json over loopback http, read only, for a face in a browser
db selftest [flags]        check this engine's own source: comments outside the header, and any call that turns a missing value into a default
```

Every command also takes:

- `--db <DB>` — the DB directory holding db.toml; found upward from the working directory, or from DB_DIR, when it is not given
- `--strict` — before ls, show and find, compare the index against a digest of every source byte instead of the length and modification time of every source file
- `--dump-commands` — print this command tree and the exit codes as json, for the README sync gate

`db gate`:

- `--detail` — also print, for each gate that counts one, the tally by attribute and document
- `--json` — print the wire json a face consumes instead of the table a person reads

`db bands`:

- `--json` — print the wire json a face consumes instead of the table a person reads

`db ls`:

- `--band <BAND>` — keep only the atoms of one band
- `--layer <LAYER>` — keep only the atoms declared at one layer: L0, L1 or L2
- `--role <ROLE>` — keep only the atoms of documents with one role
- `--executor <EXECUTOR>` — keep only the atoms written by one executor
- `--lod <LOD>` — how much of each atom to render: 0 headline, 1 body, 2 body with provenance and relations
- `--cursor <CURSOR>` — the exact id printed at the end of the previous page, or begin
- `--include-gaps` — also list the gap atoms, the bytes between the atoms that carry claims; they are excluded by default and always counted
- `--json` — print the wire json a face consumes instead of the text a person reads

`db show`:

- `--lod <LOD>` — how much of the atom to render: 0, 1 or 2
- `--json` — print the wire json a face consumes instead of the text a person reads

`db find`:

- `--band <BAND>` — keep only the hits in one band
- `--layer <LAYER>` — keep only the hits at one layer
- `--limit <LIMIT>` — how many hits to print, never above the cap of the layer
- `--include-gaps` — also search the gap atoms, the bytes between the atoms that carry claims; they are excluded by default and always counted
- `--json` — print the wire json a face consumes instead of the text a person reads

`db serve`:

- `--port <PORT>` — the loopback port to bind on both 127.0.0.1 and ::1

`db selftest`:

- `--path <PATH>` — the directory of rust files to scan; the crate's own src by default

## Exit codes

Three codes, one meaning each, the same meaning in every command. Derived from
`EXIT_CODES` in `crates/db/src/main.rs` by `tools/readme_sync.mjs`.

| code | meaning | when |
|---|---|---|
| **0** | answered | the command ran and returned at least one row, or every gate it ran passed |
| **1** | a gate counted a failure | a gate found a real break: an orphan document, a duplicate id, a chain break, a stale index, a comment outside the header |
| **2** | refused, or UNTESTABLE | the question was malformed or could not be asked: an unknown filter value, a projection whose first row alone is over the budget, a cursor that is not a row id, an empty answer, an empty corpus, an unreadable manifest, an index the source has moved past |

Every refusal also prints `reason: <TOKEN>` on stderr, so a caller reads a machine
token and not only prose. An empty answer is **2**, never 0: a page with no rows is
not an answer. `UNKNOWN` is a third verdict inside `gate`, never a fourth exit code —
it is counted and printed, never folded into a failure, and a run carrying one exits 2.

## The json wire

`--json` on `ls`, `show`, `find`, `bands` and `gate` prints one schema, and `db serve` returns the
same bytes over loopback HTTP for the same query. There is one serialiser; a control asserts the
two bodies are byte identical, because that equality is the only reason a face can be written
once and moved between transports later. The schema itself is declared once, in
`schema/wire.json`; `tests/wire_schema.rs` asserts the engine emits exactly that shape and
`tools/wire_schema.mjs` generates `face/src/wire.generated.ts` from it, so a field that exists on
one side and not the other turns one of those two red rather than drifting unseen.

`gate` puts a gate line where a row would be — `name`, `verdict`, `reason`, `count`,
`denominator`, `detail`, and a `breakdown` that is empty unless the gate counts one. `bands` puts
a band there — `id`, `title`, `abstract`, `documents`, `atoms` and `gaps`. Both leave `cap` `null`,
because neither has a row cap and a `0` there would read like one. The envelope `verdict` follows
the exit code, so a run carrying an `UNKNOWN` is `UNKNOWN` even when another gate in the same run
also counted a failure.

Three things the wire insists on:

- **The third value survives.** `verdict` is `TRUE`, `FALSE` or `UNKNOWN`, and so are the `layer`,
  `executor` and `evidence` of each row. An empty answer is `FALSE` — the question was asked and
  the answer is nothing. A question that could not be asked is `UNKNOWN`. They are not the same,
  and a face that renders them the same has thrown away the distinction the engine paid for.
- **The denominator travels with the rows.** `denominator.total / matched / returned / withheld /
  unscanned / gaps_excluded` is in every response, including every refusal, so "12 results" can
  never be printed without "of 79". `total` is the whole corpus the projection was drawn from and
  is `null`, never `0`, when the request was refused before anything was counted, because a corpus
  of zero and a corpus never counted are different answers. `withheld` is `matched - returned`:
  what the question matched and this page did not carry. `ls` and `find` leave out gap atoms — the
  bytes between the atoms that carry a claim — by default, because a reader asking what a band
  says does not want them; leaving them out is not the same as hiding them, since the count is
  still on the denominator line and `--include-gaps` returns them.
- **Depth only adds.** At `--lod 0` a row is its address and one line; `1` adds `content`; `2` adds
  `provenance` and `relations`. No field ever disappears as the level deepens.

`db serve` binds `127.0.0.1` and `::1` both, on purpose. A browser resolves `localhost` to `::1`
first, so a server bound only to the v4 loopback answers `ERR_CONNECTION_REFUSED` to
`http://localhost:<port>`. If the v6 bind fails it says so and keeps serving v4, rather than
claiming both. It answers `GET` only: `405` for anything else, `404` for an unknown route, `422`
for a projection that matched nothing, and a port already in use is exit 2.

## Measured

Two clocks per run, five runs, median. `ms (date)` brackets the call with `date +%s%N`; `ms (time)`
is the shell's own `time` builtin on a second call. Both run inside WSL, so neither includes the
cost of starting WSL. `tools/bench.mjs`, which is in this folder, is the harness that took these
samples and writes them to `build/bench.jsonl`; the corpus it ran against is not published, so
this table is a record of a measurement rather than one you can reproduce from this folder alone.

Corpus: a copy of the live `DB/` — 8 bands, 53 documents, 1,608,625 byte of markdown, 9,608 atoms,
10,091 journal lines. It is a copy and not the live tree on purpose: the live tree is written
while this runs, and a target that moves under the instrument is not a measurement.

There are two `grep` rows and the second one is the one that matters. `grep -rIn provenance
bands/` is the whole corpus, which is what someone who does not know where the answer lives has
to run. `grep -rIn provenance bands/decisions/` is the same search aimed at the one band that
holds the answer, which is what someone who does know runs. Comparing only against the first
flatters this engine, so both columns are printed and the aimed one is the denominator the claims
below use.

| command | ms (date) | ms (time) | bytes | rows | bytes vs grep | bytes vs aimed grep | ms vs aimed grep |
|---|---|---|---|---|---|---|---|
| `compile` | 774.86 | 813 | 882 | 7 | 57.9 | 19.4 | 229.25 |
| `bands` | 22.94 | 20 | 662 | 10 | 77.1 | 25.9 | 6.79 |
| `ls --layer L0 --cursor begin` | 11.87 | 10 | 15 816 | 102 | 3.2 | 1.1 | 3.51 |
| `ls --layer L1 --lod 1 --cursor begin` | 12.48 | 10 | 9 384 | 46 | 5.4 | 1.8 | 3.69 |
| `show <address> --lod 2` | 10.16 | 8 | 430 | 3 | 118.7 | 39.8 | 3.01 |
| `find provenance` | 8.60 | 7 | 2 052 | 12 | 24.9 | 8.4 | 2.54 |
| `find provenance --strict` | 18.83 | 16 | 2 052 | 12 | 24.9 | 8.4 | 5.57 |
| `find lean --layer L1` | 7.82 | 6 | 4 599 | 12 | 11.1 | 3.7 | 2.31 |
| `selftest` | 43.46 | 41 | 668 | 5 | 76.4 | 25.7 | 12.86 |
| `grep -rIn provenance bands/` | 5.19 | 2 | 51 047 | 157 | 1.0 | 0.3 | 1.54 |
| `grep -rIn provenance bands/decisions/` | 3.38 | 1 | 17 135 | 26 | 3.0 | 1.0 | 1.0 |
| `gate` | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured | Not measured |

The `Not measured` row is not missing data. `gate` exits 2 on that corpus — 2,093 atoms leave an
attribute undeclared, 11 carry two kinds of evidence marker at once, and 887 journal lines are in
the format this engine replaced, so their own links cannot be recomputed here — and timing a
command that did not answer would be measuring how fast it fails, so the harness refuses to put a
number there. That refusal is the same rule the engine applies to itself.

`bands` is the slowest of the read commands at 22.94 ms because it is the only one that counts the
whole corpus: one pass over every atom in every document, grouped by band. Everything else answers
from an index lookup and a page. It is still 25.9 times smaller than the aimed `grep` in bytes,
which is the axis this engine competes on.

`find --strict` is the same search with the cheap freshness comparison replaced by a digest over
every source byte: 18.83 ms against 8.60, so knowing the index still speaks for 4.6 MB of source
costs about 10 ms, and the default path costs nothing measurable against the run before that check
existed.

**The byte assumption holds against both denominators. The time assumption fails against both.**
`find` returns 22.9 times fewer bytes than a search over the whole corpus and 8.2 times fewer than
the same search aimed at the right band; `show` returns 42.7 times fewer than the aimed one. On
time `find` is 1.85 times slower than the whole-corpus `grep` and 2.71 times slower than the aimed
one. The target is half of `grep`, and it is not met against either denominator.

Against someone who already knows which file holds the answer, this engine loses on time and wins
only on how much comes back. What it wins outright is the case where nobody knows: one query over
eight bands, with the address and the byte range of each hit, instead of a guess at a path
followed by a second guess. Retrieval is not the claim.

Two causes were found for the time gap and both are fixed; naming only the first would understate
it. `ls`, `show` and `find` used to read every atom **and its body** out of the index and filter
in memory. The second was larger: `ls` and `find` also re-read and re-parsed every source markdown
file on **every query**, to obtain a manifest that comes from `db.toml` alone — which is why
`show`, which never did that, used to be the fastest of the three. What is left after both fixes
is small and not a defect to fix by trying harder: a process that starts, opens the index, checks
that the index still speaks for the source and then counts, ranks and renders costs about 9 ms;
`grep` at 2 to 3 ms over 1.5 MB is simply very fast at this size. Whether the crossing point is a
larger corpus is **not measured** here.

The `ls` rows and bytes are lower than an earlier run of the same commands because gap atoms are
no longer listed by default — a change in what is asked for, not a speed-up, and the row counts
above are of atoms that carry a claim, not of every byte in the corpus.

Numbers carried from earlier runs of the previous engine (a 26x total ratio over five questions)
are **not** re-measured here and are not repeated. The table above is the whole of what has been
measured by this code.

## What this code refuses to do

Most of the rules below exist because the previous version of this engine did the opposite, and an
audit of its own source caught it. They are worth stating because each one is a place where the
convenient behaviour and the correct behaviour differ.

- **It never repairs a manifest.** A `db.toml` that will not parse, declares a schema it does not
  know, sets a cap below 1, or lists a band twice is refused with exit 2, and the file on disk is
  left byte for byte alone. The older code deleted the file and wrote defaults over it, after which
  every gate passed over a configuration nobody had written.
- **An empty scan is never green.** Zero atoms, zero documents, zero readable files, a gate whose
  controls could not be built — all of it is `UNKNOWN` and exit 2, and `UNKNOWN` is counted and
  printed rather than folded into a failure. A gate that finds nothing to look at has not passed.
- **The journal's last record is covered.** `HEAD` is the fold of every line including the last one,
  so editing any record — the tail included — breaks the chain. A journal that also holds records
  written by the engine this one replaced is still checked against `HEAD`: those records carry a
  `prev_hash` this engine cannot recompute, which makes their own links `UNKNOWN`, and it does not
  make the file unchecked. Before this was fixed, one such record anywhere in the journal returned
  that `UNKNOWN` before `HEAD` was compared, and with it a count of the tampered lines among the
  ones "checked and hold" — a third value returned early is a place to hide.
- **It will not answer from an index the source has moved past.** Before `ls`, `show` or `find`
  reads a row, the length and modification time of every declared document, every `band.toml`,
  `db.toml` and the journal are folded and compared against what `db compile` recorded; if that
  disagrees, the bytes themselves are digested before anything is called stale, so a touched
  modification time alone cannot fake staleness. A stale index refuses with `STALE_INDEX` and exit
  2 rather than answering, because the answer that hurts is not the wrong row — it is the `EMPTY`
  that reads as "the corpus does not say that". `--strict` skips the cheap comparison and digests
  the bytes every time.
- **Every gate carries its own controls.** Each one runs a negative case that must fail, a positive
  case that must pass, and where the corpus allows it a vacuous case that must come back `UNKNOWN`,
  and binds each to an exit code and a reason token. A gate that has never gone red is not evidence.
- **A cursor is the whole id.** Not a prefix of it. A prefix silently paged from the wrong row.
- **The index is never asked what it contains.** `db compile` records a digest of the source, and
  `db gate` recomputes that digest from `db.toml`, every `band.toml`, every declared document and
  the journal, then compares. Asking the index to confirm its own freshness proves nothing.
- **Nothing is silently truncated.** A projection over its row cap refuses and offers a cursor; a
  page over the byte budget is cut to the rows that fit, says in its header how many of the rows
  the cap allowed it kept and why, and prints the cursor that continues it. What it will not do is
  print part of an atom. When the first row alone is over the whole budget there is no page to cut
  to, and that refuses too.
- **Advice beside a refusal is advice that changes the answer.** A refusal lists a filter only when
  that filter has more than one value in the projection being refused; when every row carries the
  same band, role and layer it says that no filter narrows it, rather than printing three that each
  return the same count again.
- **The source has no comments.** The only comments allowed in a `.rs` file are the two header
  lines. `db selftest` counts violations two ways and fails if the two counts disagree, and it also
  fails if any call in the crate turns a missing value into a default — the needles it searches for
  are built at run time so that the scanner cannot match itself.

## Building

This folder is its own cargo workspace. It is deliberately not a member of the workspace at
the root of this repository, so `cargo build --workspace` there does not build it and the
repository's checks do not run its controls. Build it from this directory.

It was developed and run inside WSL, because Smart App Control blocks unsigned executables on
the Windows side of that machine. Nothing in the crate depends on that; it is where the
numbers below come from.

```
cd db
CARGO_TARGET_DIR=<somewhere outside the synced tree> cargo test -p db
```

Measured once, on a fresh target directory, cargo 1.97.1, 12 cores, WSL, `--offline` against the
local registry: `cargo build --release` 35.3 s, `cargo test` 24.2 s, 51 controls passed
(48 in `tests/controls.rs`, 3 in `tests/wire_schema.rs`), 0 failed. One sample, not a median.

Keep the build cache off a OneDrive-synced path and off DrvFs; incremental writes across the mount
fail with `Cannot allocate memory` long before the disk is full.

## Licence

Apache-2.0. Copyright (c) 2026 Glovrex.

Direct dependencies, read from `cargo metadata` rather than from memory: `serde`, `serde_json`,
`sha2`, `clap` and `toml` are `MIT OR Apache-2.0`; `rusqlite` and `libsqlite3-sys` are `MIT`, and
the SQLite they bundle is public domain. Across the whole resolved graph of 64 packages, zero
declare no licence.
