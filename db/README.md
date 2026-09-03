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
Cargo.toml              workspace, one member
crates/db/
  src/atom.rs           the atom, its content id, and the three-valued fields
  src/manifest.rs       db.toml and band.toml; refuses rather than defaulting
  src/extract.rs        markdown into atoms, every byte claimed by exactly one
  src/store.rs          journal and hash chain, the sqlite index, the raw tier
  src/route.rs          filters, caps, cursors, FTS5 search, and the json wire
  src/gate.rs           every gate, each carrying its own controls
  src/serve.rs          loopback http, GET only, same json as stdout
  src/main.rs           the command line
  tests/controls.rs     the controls, run against the built binary
```

Three things that sit beside this crate in the tree it was developed in are not in this
folder: `superseded/`, two files replaced by `gate.rs` and `db.toml`; `tools/readme_sync.mjs`,
which derives the two sections below marked as derived; and `tools/bench.mjs`, which produced
the Measured table. The crate builds and its controls pass without them, which is what the
next section is for, but it does mean the derived sections here cannot be regenerated from
this folder alone.

## Commands

Derived from the command tree; the script that derives it is not in this folder, but
`db --dump-commands` prints the same tree as json. Edit the `#[command]` and `#[arg]` help
text in `crates/db/src/main.rs`, never this section.

```
db compile                 read the source, build the semantic index, print the counts and the digests
db push                    record the atoms that changed as admission events in the journal, then compile
db gate                    run every source, index and query gate over this DB and print pass, fail and UNKNOWN
db ls [flags]              list the atoms of a projection
db show <ADDRESS> [flags]  print one atom named by its id or by its exact address
db find <NEEDLE> [flags]   search the full text index and return addresses and scores, never bodies
db serve [flags]           answer the same wire json over loopback http, read only, for a face in a browser
db selftest [flags]        check this engine's own source: comments outside the header, and any call that turns a missing value into a default
```

Every command also takes:

- `--db <DB>` — the DB directory holding db.toml; found upward from the working directory, or from DB_DIR, when it is not given
- `--dump-commands` — print this command tree and the exit codes as json, for the README sync gate

`db ls`:

- `--band <BAND>` — keep only the atoms of one band
- `--layer <LAYER>` — keep only the atoms declared at one layer: L0, L1 or L2
- `--role <ROLE>` — keep only the atoms of documents with one role
- `--executor <EXECUTOR>` — keep only the atoms written by one executor
- `--lod <LOD>` — how much of each atom to render: 0 headline, 1 body, 2 body with provenance and relations
- `--cursor <CURSOR>` — the exact id printed at the end of the previous page, or begin
- `--json` — print the wire json a face consumes instead of the text a person reads

`db show`:

- `--lod <LOD>` — how much of the atom to render: 0, 1 or 2
- `--json` — print the wire json a face consumes instead of the text a person reads

`db find`:

- `--band <BAND>` — keep only the hits in one band
- `--layer <LAYER>` — keep only the hits at one layer
- `--limit <LIMIT>` — how many hits to print, never above the cap of the layer
- `--json` — print the wire json a face consumes instead of the text a person reads

`db serve`:

- `--port <PORT>` — the loopback port to bind on both 127.0.0.1 and ::1

`db selftest`:

- `--path <PATH>` — the directory of rust files to scan; the crate's own src by default

## Exit codes

Three codes, one meaning each, the same meaning in every command. Derived from
`EXIT_CODES` in `crates/db/src/main.rs`, which `db --dump-commands` also prints.

| code | meaning | when |
|---|---|---|
| **0** | answered | the command ran and returned at least one row, or every gate it ran passed |
| **1** | a gate counted a failure | a gate found a real break: an orphan document, a duplicate id, a chain break, a stale index, a comment outside the header |
| **2** | refused, or UNTESTABLE | the question was malformed or could not be asked: an unknown filter value, a projection over its cap or budget, a cursor that is not a row id, an empty answer, an empty corpus, an unreadable manifest |

Every refusal also prints `reason: <TOKEN>` on stderr, so a caller reads a machine
token and not only prose. An empty answer is **2**, never 0: a page with no rows is
not an answer. `UNKNOWN` is a third verdict inside `gate`, never a fourth exit code —
it is counted and printed, never folded into a failure, and a run carrying one exits 2.

## The json wire

`--json` on `ls`, `show` and `find` prints one schema, and `db serve` returns the same bytes over
loopback HTTP for the same query. There is one serialiser; a control asserts the two bodies are
byte identical, because that equality is the only reason a face can be written once and moved
between transports later.

Three things the wire insists on:

- **The third value survives.** `verdict` is `TRUE`, `FALSE` or `UNKNOWN`, and so are the `layer`,
  `executor` and `evidence` of each row. An empty answer is `FALSE` — the question was asked and
  the answer is nothing. A question that could not be asked is `UNKNOWN`. They are not the same,
  and a face that renders them the same has thrown away the distinction the engine paid for.
- **The denominator travels with the rows.** `denominator.matched / returned / unscanned` is in
  every response, so "12 results" can never be printed without "of 79".
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
cost of starting WSL. The harness that took these samples is not in this folder, and the
corpus it ran against is not published, so this table is a record of a measurement rather
than one you can reproduce from here.

Corpus: a copy of the live `DB/` — 7 bands, 36 documents, 1,063,436 byte of markdown, 6,462 atoms.
`grep` is `grep -rIn provenance bands/` over the same tree, and the `bytes vs grep` column is how
many times less it returned.

| command | ms (date) | ms (time) | bytes | rows | bytes vs grep |
|---|---|---|---|---|---|
| `compile` | 626.25 | 579 | 787 | 7 | 37.3 |
| `ls --layer L1 --lod 1 --cursor begin` | 71.75 | 73 | 7 436 | 68 | 3.9 |
| `show <address> --lod 2` | 35.13 | 33 | 325 | 4 | 90.3 |
| `find provenance` | 66.83 | 62 | 1 992 | 12 | 14.7 |
| `find provenance --layer L1` | 61.78 | 60 | 2 933 | 12 | 10.0 |
| `selftest` | 41.66 | 41 | 668 | 5 | 44.0 |
| `grep -rIn provenance bands/` | 4.35 | 2 | 29 361 | 101 | 1.0 |
| `gate` | Not measured | Not measured | Not measured | Not measured | Not measured |
| `ls --layer L0 --cursor begin` | Not measured | Not measured | Not measured | Not measured | Not measured |

The two `Not measured` rows are not missing data. `gate` exits 1 on that corpus and
`ls --band decisions --layer L0` exits 2 because that band declares no L0 atom; timing a command
that did not answer would be measuring how fast it fails, so the harness refuses to put a number
there. That refusal is the same rule the engine applies to itself.

**The byte assumption holds and the time assumption does not.** `find` returns 14.7 times fewer
bytes than `grep` over the same corpus, and `show` 90 times fewer — but `find` takes about 62 ms
where `grep` takes 2. The cause is in `route.rs`: `ls` and `find` read every atom and its body out
of the index before filtering, so the FTS5 query saves bytes on the way out but not work on the way
in. That is a known defect with a known fix — push the filter into SQL — and until it is fixed the
honest claim is bytes, not milliseconds.

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
  so editing any record — the tail included — breaks the chain. Before, the tail had no successor
  and could be rewritten unnoticed.
- **Every gate carries its own controls.** Each one runs a negative case that must fail, a positive
  case that must pass, and where the corpus allows it a vacuous case that must come back `UNKNOWN`,
  and binds each to an exit code and a reason token. A gate that has never gone red is not evidence.
- **A cursor is the whole id.** Not a prefix of it. A prefix silently paged from the wrong row.
- **The index is never asked what it contains.** `db compile` records a digest of the source, and
  `db gate` recomputes that digest from `db.toml`, every `band.toml`, every declared document and
  the journal, then compares. Asking the index to confirm its own freshness proves nothing.
- **Nothing is silently truncated.** A projection over its row cap or byte budget refuses, prints
  what would narrow it, and offers a cursor.
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

Measured once, on a fresh target directory, cargo 1.97.1, 12 cores, WSL, dependencies already
in the local registry: `cargo build --release` 20.9 s, `cargo test` 15.1 s, 21 controls
passed, 0 failed. One sample, not a median.

Keep the build cache off a OneDrive-synced path and off DrvFs; incremental writes across the mount
fail with `Cannot allocate memory` long before the disk is full.

## Licence

Apache-2.0. Copyright (c) 2026 Glovrex.

Direct dependencies, read from `cargo metadata` rather than from memory: `serde`, `serde_json`,
`sha2`, `clap` and `toml` are `MIT OR Apache-2.0`; `rusqlite` and `libsqlite3-sys` are `MIT`, and
the SQLite they bundle is public domain. Across the whole resolved graph of 64 packages, zero
declare no licence.
