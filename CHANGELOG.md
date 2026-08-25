# Changelog

**Status**: new in v0.2.4, 2026-08-13 (`req/38` §68 #8 ruling = `req/111` §requires-ruling **B-7** adopted). (sem: SEM-CHANGELOG.md-001)

## R35 — the road that writes says so from every verb that walks it, not only from `gx serve` (2026-08-21, `req/38` §264, `req/470`)

### Changed

- **The 43 §7-3c sentence R34 added is no longer `gx serve`'s alone.** It is printed by
  `gx verify`, `gx commit`, `gx undo`, `gx repair --yes` and `gx serve`, each under its own name,
  and it reaches an agent's operator through `gx wrap` as well. Audit 34 counted `Engine::recover`
  at six shipped call sites against one caller of the sentence, then drove the four that had none
  over a crash with a third party's bytes in the file: all four replaced those bytes, and three of
  them printed **nothing at all** (`gx verify` did it at `rc 0`, answering `Admit`).
- **The sentence goes to stderr and names its verb.** Its prefix was the literal `"gx serve: "`
  baked into the format string, so moving the function without moving the name would have had
  `gx verify` announce itself as `gx serve`.
- **`docs/LIMITS.md`'s v0.5-t paragraph is corrected in the same commit as the repair.** It claimed
  "it changes all of it. Every row that walks that road now prints a sentence" twenty lines after
  the same page had measured `gx repair` printing nothing on stderr. The old sentence is kept in
  its own tense, because how that claim came to be made is the most useful thing on the page.

### Note

- Nothing about what gx *knows* changed. On this road it still cannot tell its own unrecorded apply
  from somebody else's write, for the reason `docs/LIMITS.md` gives (`req/78` §3.2 Λ4, and no
  post-apply fingerprint in the journal vocabulary). The repair is to the telling, and this time
  that claim is measured across all five verbs rather than asserted from one.
## DR-46-31 — an escalated commit's receipt can be re-issued (2026-08-21, `req/38` §261 ruling 2b, `req/473`)

### Changed

- 🔴 **`EngineJournalRecord::HumanDecision` gains `verdict_digest: Option<Cid>`, and
  `gx repair --yes --reissue-receipts` stops refusing every escalated commit.** `Engine::escalation`
  digests the ruling and issues both the T-5 verdict receipt and the later `CommitReceipt` under
  that digest; the journal carried the ruling's `kind`, `reason` and `actor` and not the value taken
  over them, so replay had nothing to move `StateRow.verdict_digest` with and left T-4c's `Escalate`
  proof in the seat. Σ then named the person's `Admit` beside the escalation's digest, the rebuilt
  payload could not reproduce the leaf, and the re-issue answered `world_moved` about an untouched
  substrate — for **every** commit that walked E-M3-4's road, which is every commit with no
  constructible inverse. Raised by `req/453` §10, confirmed at `crates/gx-engine/src/replay.rs`
  by `req/470` §4-3, numbered by `req/38` §261 ruling 2b.
- **This does not move the receipt wire and does not move Σ's shape.** `StateRow.verdict_digest` is
  a seat that already existed; what changed is its **value** on rows that went through T-5. The
  journal field is `serde(default)` and omitted when absent, in `InverseEscrowed.reads`' shape
  (E-M5-13's precedent), so records written before this release decode as they always did **and
  re-encode to the same bytes**. No golden vector is regenerated and none is added.
- **Journals written before this release are not repaired, deliberately.** A `HumanDecision` with no
  digest replays to the old degradation and its re-issue is still refused. The value is re-derivable
  from the record's other three fields and gx declines to derive it: a derivation would claim the
  digest a receipt was signed under while computing a fresh one, and the two diverge silently the
  day `HumanRuling` or its canonical encoding moves. `docs/LIMITS.md` v0.5-u states the boundary.
- **42 §3.13 gains the field as a canon addition** (`E-DR4631-1`, the field-shaped erratum form
  ruled in `req/38` §265 ruling 2), with the old spelling kept verbatim beneath it.

## R34 — 43 §7-3c's road says what it wrote, and an `Aborted` recovery stops being called a resume (2026-08-21, `req/38` §254, `req/449`)

### Changed

- **`gx serve`'s start-up line splits `resumed` into the four roads it sums**, at the granularity
  `gx repair --json` has published since R27: `closed_from_receipt`, `closed_from_leaf`,
  `ledger_held_the_commit` and `apply_was_announced`. `resumed` is unchanged and is still their
  sum, so a monitor reading the old field reads the same number. The one that matters is
  `apply_was_announced`: it is the only road of the four on which a start-up **writes to a
  substrate**.
- **A row whose apply was announced and then failed is no longer silently counted as a resume.**
  Monitoring 33 drove an adapter that performs the change and then answers an error — the shape
  `req/372` M-01 calls "the commonest real failure, where the call worked and the answer was lost
  coming back" — and the row came back `Aborted(ApplyFailed)` with `Rollback::NotAttempted`, over a
  world that had moved, counted under `resumed`, with **nothing printed**. It is now counted under
  the new `recover.announced_and_aborted` and it prints a sentence: what was announced, that the
  roll-back was not attempted, and that whether the substrate moved is a question this process
  cannot answer (req/449 H-02).
- **43 §7-3c's road now says, out loud, that it applied the delta without comparing anything.**
  R33 made §7-3b refuse rather than overwrite a world that moved after the crash. On §7-3c — no
  leaf in the ledger — the delta is still applied, over whatever the substrate holds at that
  moment, and monitoring 33 measured a third party's file being replaced by an `rc=0` start-up
  answering `refused: 0` and `/healthz` `ok` in silence. **The behaviour is unchanged and the
  silence is not**: the gate that would have detected the third party does not exist and cannot be
  built from this journal — `ApplyStarted` carries `{transformation, delta_cid, at}` and no
  fingerprint, the only fingerprint recorded is `Planned.fp0`, and with an `ApplyStarted` present a
  fingerprint that differs from `fp0` is also what a **successful** apply looks like (`req/78` §3.2
  Λ4). A post-apply fingerprint record would separate them; adding one is a change to 42 §3.13,
  raised as **M5H5-3** and deferred to its own DR. Until then the road declares what it did and
  what it did not check (req/449 H-01).
- **`RECOVERY_REBUILD_DISAGREES` lists a fourth cause.** R33 replaced an assertion with a
  three-member disjunction and monitoring 33 drove a bed in which all three members are false: an
  adapter that digests *what it was sent* on write and *what the substrate holds* on read — every
  server that normalises, re-encodes or stamps — reaches the refusal on an untouched project, with
  the signing key, with the world where the commit left it. The direction is fail-closed and
  `docs/LIMITS.md` v0.5-s declared it; the sentence did not. It now names it, and says that under
  that cause the refusal is permanent under re-running (req/449 M-01).

### Fixed

- **`tools/gates/pack_format_gate.sh` checks content rather than existence.** All three gated
  conditions used to pass when the *way* of breaking them changed: a line 1 that *mentions* the
  SPDX identifier while line 2 declares another licence; a scenario file that is `{}`, zero bytes,
  or a **directory**; and a `custom:` tag no `ShippedPack` row declares. That last one was one
  constant deep — `substrate: POSTGRES_SUBSTRATE_TAG` in `packs.rs` satisfied a `grep` that never
  mentioned the tag being checked, which switched the "no row declares this" half off for **every**
  pack. F8 now matches line 1 exactly, F1 parses the scenario and requires at least one case, and
  F4 resolves the declared tag set once (constants followed to their definition; an unresolvable
  one is a red, not a pass). A directory under `policies/` holding neither a `.cedar` nor a
  `.json` is no longer judged as a pack — the gate's one false red (req/449 M-02).
- A doubled clause in `gx-engine`'s published rustdoc: "the key case that *can* be established is
  established is established" (req/449 L-03).

## R33 — 43 §7-3b's recovery reads the world instead of re-applying to it (2026-08-21, `req/38` §249, `req/443`)

### Changed

- 🔴 **A receipt now says which of C-25's three answers this change got, and the escrow row has a
  seventh word with a writer.** `SubstrateAdapter::invert` returns an `InvertOutcome` — the inverse,
  what the escrow read to build it, and the verdict — instead of a bare `Option`, and the two facts
  that used to be computed and then dropped at the crate boundary now reach the signed bytes.
  Concretely: `ReceiptPayload` gains `read_set` values where it previously carried `None` at every
  commit, and a fourteenth field `reversibility`; `InverseStatus::Undetermined` is written when a
  declared read did not answer under `"$on_read_failure": "unknown"`, where `Unavailable` was
  written before. **This moves the wire**: a canonical map with one more key is different bytes even
  when the value is absent, so a receipt issued by this release digests differently from one issued
  by the last. The pre-change golden vectors are kept beside the new ones and the difference is
  asserted as a subtraction rather than regenerated
  (`crates/gx-witness/tests/inverse_status_wire.rs`). The trait still has exactly seven methods.
  (`req/38` §258 / E-DR4626-1, closing DR-46-13 and the producer half of DR-46-24(A).)
- **`gx repair --yes` and `gx serve`'s start-up recovery no longer call `adapter.apply` on the road
  where the ledger already holds the row's leaf.** That road is 43 §7-3b's crash window, and a leaf
  in the ledger is proof the apply completed before the crash (`Engine::commit` reaches the ledger
  after `apply_once` returns), so 42 §3.10's `postcondition_fingerprint` is now **read** off the
  substrate rather than produced by re-applying the delta. Monitoring 32 measured the old order
  writing first and refusing second: `adapter_apply_calls=1` on an untampered project recovered
  under a second key, and an operator's file going from `two` back to `one` on a journal whose open
  row had been re-pointed at a delta the project already held (req/397 H-01, repaired by R33).
- **A §7-3b row whose world has moved since the crash is now refused rather than overwritten.** It
  used to be silently re-applied over and then closed. The refusal carries a new sentence,
  `RECOVERY_REBUILD_DISAGREES`, which lists the three causes a digest comparison cannot tell apart
  instead of asserting one; `RECOVERY_KEY_MISMATCH` is now printed only where the project's recorded
  head names a different key, which is a measurement rather than a guess.
- **`gx serve`'s start-up JSON `recover.refused` means what it says.** Its source defined a non-zero
  value as "the substrate was **not** written to" while the road that produced it had just written;
  the definition is corrected, and the `resumed` line no longer claims "the adapter was asked again
  for each" (two of the four paths it folds never asked an adapter anything, and since R33 a third
  does not either).
- **The CLI's journal-marker stamp is made durable** — `write_all` + `sync_all` + a parent-directory
  fsync, the order the engine's own stamp has used since R31.

## R32 — a zero-byte journal is reported as what the disk holds, and diagnoses name the term that failed (2026-08-21, `req/38` §250, `req/399`)

### Changed

- **`gx repair` on a `journal_format=chained` project whose journal is 0 bytes now exits 1** (was 0):
  the reader no longer answers for a marker the writer has not yet stamped, so a wholly missing
  journal body is a downgrade, not an intact chain. Diagnoses under `journal_intact` now name
  which of the seven terms departed (`JournalDeparture`), and a refusal never carries a clause
  that is false for its own condition (req/392 M-01/M-02, repaired by R32).
## DR-46-24(A) — the receipt says what the escrow read, and at which granularity (2026-08-21, `req/38` §236 ruling 2, `req/440`)

### Changed (breaking wire change)

- **`ReceiptPayload` gains two fields, so every receipt encodes to different bytes and to a
  different ledger digest.** A canonical DAG-CBOR map with two more keys is not the map it was, and
  an absent value is `0xf6` **at a key** rather than nothing — so unlike E-M5-11 ("the wire did not
  move") this is a migration. A receipt issued before this release has a ledger leaf keyed on the
  old digest and re-issuing the same transformation now produces a new one. The pre-change golden is
  kept beside the new one in `crates/gx-witness/tests/receipt_verdict_wire.rs`, and the test that
  subtracts the two new keys out of the shipped bytes and requires the old bytes back is what says
  nothing else moved.
- **`read_set`**: the objects gx read to build the inverse it escrowed, carried at one of two
  granularities with the tag inside the field — the entries themselves up to five distinct objects
  (`G3`, decidable from the receipt alone), a Merkle root past that (`G4`, flat in the count and
  decidable only with the entries from beside the receipt). `None` on a `VerdictReceipt`, always:
  the escrow reads at 43 T-10b, during commit. **It is gx's read and not the agent's** — the
  cross-object question is DR-46-25's and this field does not answer it (`docs/LIMITS.md`).
- **`fingerprint_scope`**: what the pre- and post-state fingerprints were taken over (42 §3.5's
  `scope`). `FingerprintBytes` is unchanged — still the opaque thirty-two bytes E-M2-2 made it.

### Added

- **`InverseStatus::Undetermined`** (DR-46-13, folded in by §237-5): "nobody established whether an
  inverse exists", which `Unavailable` ("`invert()` answered `None`") had been carrying as well.
  Additive; the six values before it did not move a letter. **It has no producer yet** and the
  blocker is named in the type: `SubstrateAdapter::invert` returns `Result<Option<PlannedDelta>>`,
  so C-25's third answer has nowhere to travel until that declaration widens.

## R16 — the window of that count is the binary, not a crate (2026-08-18, `req/38` §192 ruling 2, `req/262`)

### Fixed

- **The census of "every road to a standard stream" had been taken over one crate, and `gx` is
  built from fourteen.** R15's predicate was right — count destinations, not payloads — and its
  window was `crates/gx-cli/src/`. The sixteenth audit measured **six** `eprintln!` sites in
  `gx-api`, three of them on `gx serve`'s request road; widening the window to the artefact (the
  crates the binary's manifest reaches through `[dependencies]`) found **seven more** in
  `gx-mcp-wire`, the wire `gx wrap` puts an agent behind. **Thirteen**, all of which crash the
  process on a failed write. Measured, three runs each, no variation: with `.gx/drafts` at mode
  `0500` and `gx serve 2>/dev/full`, `POST /v1/candidates` answered **0 bytes with no HTTP status
  line** and the connection closed, where the same request on the same project with `2>/dev/null`
  answered **201 Created** (875 bytes). The control — a healthy project with a dead error stream —
  answered `201` both ways, so the fault is the composition, and both halves arrive together on a
  full disk. The server stayed up and answered the next request, so nothing monitored notices; what
  is lost is the request's **answer**. `req/262` H-01. Fixed: `gx_api::notes` and
  `gx_mcp_wire::notes` are typed roads with the same shape as `gx_cli::emit`, `main::settled` adds
  all three counters, and `declaration_writer_doubt` D-6 **computes** the window from the manifests
  rather than being handed a list — a crate joins the census the day its manifest line is written.
- **A project every `gx` verb refused could still be served.** `Layout::create` has asked the shape
  of every declared directory since R14; `Layout::open`, the door `gx serve` uses, asked nothing.
  With one byte where `.gx/receipts` belongs: `gx submit` exit 1 `LAYOUT_BLOCKED`, and `gx serve`
  **started**. The HTTP commit road then answered `201` → `200` → `500 INTERNAL` with the target
  file already rewritten, one leaf on the ledger, zero commits in the journal, no commit receipt,
  and `gx undo` refusing — the effect applied and the document that proves it lost. `req/262` M-01.
  Fixed: both doors ask the same question first, and `gx serve` refuses to start with the CLI's own
  word. The comparison with a degraded server is written down in `layout.rs` and 44's v0.5-c note.
- **gx named a command that does not exist, a nature its own declaration contradicts, and a cause
  it had not measured.** The `500` above said `gx receipt export` refiles the receipt (`gx receipt`
  has `show` and `verify`: `error: unrecognized subcommand 'export'`, exit 1) and called
  `.gx/receipts/` req/56 §2's `Derived` (`GX_PATHS` says `Nature::Source`, with the reason beside
  it). `gx-engine`'s own message ended "What to fix: the write permission on `.gx/receipts/`, or
  the disk it is on" where the operating system had said `File exists (os error 17)`. `req/262`
  M-01. Fixed: the real road (`gx repair --yes --signing-key <KEY_ID>`, `--reissue-receipts`
  beside it), the real nature, and a pointer to what the operating system said instead of a guess.
- **The machine that checks whether a remedy is true was running a command of its own.** 43 §7.17
  (b) condition 2 says the truth of a remedy is measured by running what it tells you to run, and
  R15's gate ran `gx repair --yes --signing-key <ID>` while the message said `gx repair --yes`.
  Run as printed, on seven directories in two shapes: **fourteen out of fourteen** answered
  `cleared: false`, `kept_aside: []`, and the next `gx submit` refused again. `req/262` M-02.
  Fixed: the message carries `--signing-key <KEY_ID>` and why a key is needed, and the gate now
  **extracts the command line out of the message** and runs it with nothing added — measured
  0 of 7 on the previous binary and 7 of 7 on this one. The key requirement itself is kept, and the
  alternative (splitting `writing` into "needs a key" and "does not") is argued down in `layout.rs`.

### Changed

- `gx serve` refuses to start on a project whose `.gx/` has a file, or a symbolic link, where a
  declared directory belongs. It started before. The way out is printed with the refusal.

## R15 — the count of what a stream carries is a count of destinations (2026-08-18, `req/38` §188 ruling 2, `req/259`)

### Fixed

- **"Every stream that carries an answer" had been implemented as "every site carrying one kind of
  object".** R14 moved 44 §1.3's problem object onto `emit::problem_line` and wrote the census
  predicate as `.problem()` — a **payload**. Counted by destination instead, `crates/gx-cli/src/`
  still held **forty-three** `eprintln!` sites, every one of which ends the whole run at exit
  **101** when the destination refuses, whatever it was carrying. Two of them were answers by any
  reading, and the two arms a buyer meets first were measured three runs each, no variation:
  `gx key gen --json 2>/dev/full` — exit **101**, stdout **0 bytes**, and the secret key already
  written to `.gx/keys/`, so the operator held a key that nothing had named — and
  `gx wrap … 2>/dev/full`, the product's membrane, whose start-up JSON and session summary are on
  stderr by design because stdout carries the agent's protocol frames. `gx demo` stopped after step
  1 of 3 for the same reason. `req/259` H-01.
- **The exit from a blocked declared directory existed for one directory out of seven.** R14
  generalised the **refusal** (`Layout::create` pre-scans every `Shape::Dir` row of `GX_PATHS`) and
  left the **exit** at a hard-coded `REPAIR_DIR = "repair"`. Measured on all seven, three runs
  each: with a regular file at `.gx/evidence`, `.gx/index`, `.gx/drafts` or `.gx/receipts`,
  `gx submit` refused `LAYOUT_BLOCKED` permanently while `gx repair` answered exit **0**,
  `remedy: null`, `repair_dir_blocked: null` — the verb whose job is to say what is wrong reported
  the project healthy — and `gx repair --yes` set nothing aside. `.gx/checkpoints` exited 1 with no
  way out. `.gx/ledger` came back as `HISTORY_LOST`, a word about lost contents, because the shape
  was asked **after** the existence questions. `req/259` M-01.
- **The remedy gx handed the operator was false for six of those seven.** The problem object said,
  word for word, that `gx repair --yes` "renames it to `.gx/<name>.pre-repair.<n>`, makes the
  directory, and names the copy it kept under `kept_aside`" — three clauses, none of which held
  outside `.gx/repair`. `req/227` M-04 ("a remedy that names the wrong file is worse than none") is
  the rule R14 itself cited when repairing `OUTPUT_FAILED`'s sentence. `req/259` M-01.
- **`gx wrap`'s session summary was delivered with the answer discarded.** `let mut err =
  std::io::stderr(); let _ = writeln!(err, "{summary}");` — the third instance of the class R13
  closed for stdout and R14 closed for the problem object. It was never measured alone, because the
  start-up line's panic came first; it is reported as code rather than as a measurement.
  `req/259` M-02.
- **44 §2.3's stacked table had no row for `BUSY`.** The refusal vocabulary is 12 base codes plus
  `RULED_ADDITIONS`; Rust and the TypeScript SDK both said **25** and the specification's tables
  said **24**. `BUSY` arrived with DR-43-2, has three paragraphs of v0.4-m prose arguing for it, and
  had no line. The consequence was exact and small: `LAYOUT_BLOCKED` was written up as the additive
  table's thirteenth row when it was the twelfth. `req/259` L-01.
- **`gx demo`'s arrival log threw away its own write.** `let _ = writeln!(file, "call\t…")` in the
  demo notes server — the log `gx demo` reads to say how many calls actually reached the server. A
  failed write left the walk one arrival short and told nobody. `req/259` L-02.

### Changed

- **Nothing in `crates/gx-cli/src/` writes to a standard stream except `gx_cli::emit`.** The
  answer/sentence distinction is gone from that module: two writes to the same file descriptor fail
  together, and sorting them by meaning is a judgement that has to be re-argued whenever a verb
  learns a new sentence — it was re-argued wrongly twice. `std::io::stderr()` is held in one module;
  `std::io::stdout()` is held there and at three declared sites (two MCP transports, where the
  stream *is* the wire, and `main`'s one delivery through `Outcome::emit`).
- **A note that could not be delivered does not move the exit status**, and that is written down in
  one place (`main::settled`) rather than discarded at forty-three call sites. `2>/dev/null` and
  `2>/dev/full` are two ways of throwing stderr away, and a run that answered them differently is a
  run no script can branch on — R14's reasoning for a refusal, applied to something with strictly
  less claim on the status than a refusal has. There is no third stream, and 44 §1.3 fixes what
  stdout carries, so borrowing it would corrupt an answer in order to report a note.
- **`gx key list` prints `public_key` beside `key_id`.** The harm in `gx key gen`'s crash was not
  only the panic: the two strings that name a key existed **only** on the standard output that never
  arrived, while the secret sat on the disk. The store holds the seed and this verb already opens
  each file to answer `key_id_inside`, so both public halves are derivable there. Nothing new is
  exposed — a public key is the half that is published, and the secret's **location** is still
  carried by the one sentence on stderr (44 §1.2's "stdout is two fields" is unmoved).
- **`gx repair`'s `repair_dir_blocked` is a list.** `null`, or one `{path, cleared, kept, why}` per
  blocked declared directory, in `GX_PATHS` order, because a restore can put files at two of them at
  once. The report's key set is **51**, unchanged, and all three reports still carry the keys in the
  same order.
- **The shape of a declared directory is asked before the project is asked whether it has a
  history.** R14's pre-scan sat in front of every write and behind every read, so a file at
  `.gx/ledger` produced `HISTORY_LOST` — the sentence for a project whose log is gone — instead of
  `LAYOUT_BLOCKED`, the sentence for a path something else is sitting on. Clearing the blockage
  restores the **shape** and not the contents, and the remedy now says so: if what belonged in that
  directory was the log or the receipts, the next refusal names that loss by itself.
- **`kept_aside`'s stems and `gx repair --yes`'s subject are read off `GX_PATHS`.** One reading of
  req/56 §2's table serves the door that refuses, the verb that clears and the report that counts,
  through `gx_cli::layout::declared_directories`. The rule is unchanged: only names gx itself can
  produce are counted, so an operator's `.gx/notes.pre-repair.3` is still not one (`req/244` L-01).
- **44 §2.3 gains the row for `BUSY`** (503 with `Retry-After: 1` and `retry_after_ms`, CLI **1**).
  Not a new code, a missing line: the meaning, status, exit and member are v0.4-m's and do not move.
  The three faces of the vocabulary are **25 / 25 / 25**.

### Added

- `gx_cli::emit::note_line` (`std::io::Result<()>`) and `gx_cli::emit::note`, the road the
  forty-three sentences take, with `gx_cli::emit::notes_undelivered` as the count `main` reads once.
- Four gates in `crates/gx-cli/tests/model_a_probes.rs` (**32 → 36**), all four red on the previous
  binary: the exit of a verb whose answer rides stderr, measured against `/dev/full` and
  `/dev/null` and required to agree; a generated key named again out of the store; every declared
  directory blocked by a file taking the same road out; and the remedy checked by **running what it
  tells the operator to run**.
- One census in `probes/doubt/tests/m6_gx_code.rs` (**6 → 7**) that counts 44 §2.3's stacked rows
  against `RULED_ADDITIONS` — the third face of the vocabulary, which had no machine holding it.
- `probes/doubt/tests/declaration_writer_doubt.rs` D-6 is rewritten as a **destination** census:
  no `print!`/`eprint!` family macro under `crates/gx-cli/src/` outside `emit.rs`, `std::io::stderr()`
  nowhere else, `std::io::stdout()` only at the three declared sites, and no `let _ = write!` left
  anywhere. The one excluded write is named in the source with its reason — clap's own usage error,
  which runs before there is a verb to speak for, and which was measured at the same exit status
  with stderr thrown away either way, on both binaries.
- 43 §7.17 (three stacked articles) and `docs/LIMITS.md` **v0.5-b**, which corrects v0.5-a's
  sentence "Both streams answer for their writes now."

## R14 — a report is delivered on every stream it is put on (2026-08-18, `req/38` §186 ruling 2, `req/246`)

### Fixed

- **The delivery was closed on stdout and left open on stderr, which is the wider of the two.**
  R13 made `Outcome::emit` answer for the write and the flush and turned a failed delivery into 44
  §1.3's problem object under `OUTPUT_FAILED`. That object was printed by `eprintln!`, and Rust's
  `eprint!` family returns nothing exactly as `print!` does — so it **panics** on a write error.
  The same macro carried **every verb's every refusal**, which is what 44 §1.3 puts on that stream.
  Five arms, three runs each, exit **101** with a Rust panic string in all fifteen: `gx receipt show
  gx1:doesnotexist 2>/dev/full` (a **read** verb, healthy stdout, nothing written into `.gx/`),
  `gx submit` refusing `CONFIG_ABSENT` the same way, `gx repair --yes > /dev/full 2>/dev/full`,
  `gx repair --yes 2>&1 | true`, and `gx limits > /dev/full 2>/dev/full`. clap's own usage error
  stayed at **1** under the same conditions, which is how the finding is known to be about this road
  rather than about the process. `req/246` H-01.
- **`gx repair --yes` filed no record on the one road R13 built for it.** A project that lost
  `.gx/config.toml` and `.gx/ledger/journal` together has `--yes` write the settings back (R13,
  `req/244` H-02) — 139 bytes, measured — and filed nothing, so the next `gx repair` answered
  `previous_repair: null` about a run that had put bytes on the disk. The `OUTPUT_FAILED` object
  told that same run, with no condition on the sentence, to go and read `.gx/repair/last.json`.
  `req/246` M-01.
- **A project the writer refused by name was called healthy by the report.** R13's `HISTORY_LOST`
  refuses a project with entries under `.gx/index/`, `.gx/evidence/` or `.gx/drafts/` and none of
  `Layout::logged`'s three witnesses. `repair::report_without_engine` asked only the three witnesses,
  so the same project got exit **0** and "this is what `.gx/` looks like after `gx key gen` in a
  fresh directory" — three runs, no variation — while `gx submit` was refusing it. `req/246` M-02.
- **`.gx/repair/last.json` grew a generation per run and destroyed itself at 127.** The filed copy
  is the printed report, and the printed report carries `previous_repair`: the whole of the report
  the run before it filed. On one healthy project with nothing to repair and no adversary: 1 run
  1,718 B, 10 runs 23,444 B, 40 runs 177,764 B, 100 runs 864,404 B, 126 runs **1,318,468 B** — and
  at **127** `serde_json`'s recursion limit refused the read, `previous_repair` became `None`, and
  the file was rewritten from an empty history, so "no repair has run here" and "126 have" became
  one answer. The printed report grew with it (176,584 B at run 126), past the 64 KiB a pipe holds,
  which put a truncated JSON object on stdout under `| head` against 44 §1.3. `req/246` M-03.
- **One byte in the wrong place locked a project out of gx with no exit.** R13 declared
  `.gx/repair/` as a `Shape::Dir` row, and `Layout::create`'s loop asks the operating system for
  every such row. With a regular file at that path, `gx submit`, `gx log head` and `gx receipt list`
  all refused `INTERNAL` "create …/.gx/repair: File exists (os error 17)" (three runs each,
  permanently), while `gx repair` answered exit **0**, `ledger_agrees_after: true`, `remedy: null`,
  with `repair_record.written: false` as the only trace — a key that moved neither the status nor
  the remedy. `req/246` M-04.
- **Three of the four roads that refuse in `Layout::create` still created directories on the way
  out.** 43 §7.15 (f) wrote the rule and R13 closed the one road `req/244` L-04 had measured
  (`JOURNAL_ABSENT`); `DECLARATION_ABSENT`, `DECLARATION_UNREADABLE` and `CONFIG_ABSENT` stood after
  the `Shape::Dir` loop, and each left `.gx/evidence/` and `.gx/repair/` behind it. `req/246` L-01.
- **`unwrap_or_default()` had moved from stdout to stderr rather than being removed.** Two copies in
  `main.rs`, both serialising a problem object, both printing an **empty line** at an unchanged exit
  status if the serialiser refused. No reachable input produces it today, which is not the same fact
  as "there is no road" — R13's own argument. `req/246` L-02.
- **Two buyer-facing sentences carried runs of whitespace where a line continuation belonged.**
  `Error::OutputFailed`'s text held ten spaces mid-sentence, and `gx repair`'s H-02 remedy held a
  literal newline and thirteen spaces **inside a JSON string**. `req/246` L-03.
- **`.gx/repair/last.json` was written in place rather than through a temporary name.** The writers
  are all inside `.gx/LOCK`; the report mode's read of it is not. No torn read was reproduced — the
  exposure is what is closed. `req/246` L-05.

### Changed

- **44 §2.3's ruled additions gain a thirteenth code: `LAYOUT_BLOCKED`** (500 / CLI **1**). A path
  that is not a directory sitting where one of `.gx/`'s declared directories belongs is completely
  classifiable, and `INTERNAL` is 44 §2.3's word for what is not. The word names the **predicate**
  and not the path: `Layout::create` checks the shape of every declared directory before it makes
  any of them. No new exit number is minted (`req/38` §148). The TypeScript SDK goes from
  twenty-four codes to **twenty-five**.
- **`gx repair`'s report gains a fifty-first key, `repair_dir_blocked`** — `null`, or
  `{path, cleared, kept, why}`. All three reports (healthy, journal-absent, engine-refused) carry it
  in the same position, which `crates/gx-cli/tests/model_a_probes.rs` compares as an ordered list.
- **The filed copy of a repair report holds no generations.** `.gx/repair/last.json` keeps this
  run's report with `previous_repair` reduced to a reference — `{path, bytes, taken_at, kept, why}`.
  The **printed** report is unchanged and still carries the previous run's report in full, which is
  R13's guarantee that a run whose stdout died is readable from the next command; it is now bounded,
  because the object it embeds no longer embeds another. The reference carries no digest, and that
  is deliberate: `gx-cli` may not depend on `gx-canon` (41 §6 gives the canonical encode one door,
  and a CLI that could mint a `Cid` could name a transformation the engine never saw), and no other
  hash of bytes is on this crate's dependency list.
- **`Layout::used_without_witness` is `pub(crate)`**, so the writer's door and the reporting door ask
  "has this project been used" with one predicate — 43 §7.15 (b) generalised to "the predicate that
  refuses and the predicate that decides the exit are the same predicate".
- **`kept_aside` counts a second stem, `repair.pre-repair.<n>`.** The list stays closed: an
  operator's own `.gx/notes.pre-repair.3` is still not counted (`req/244` L-01).

### Added

- `gx_cli::emit::problem_line` — 44 §1.3's problem object on stderr, through `write_all`, a
  newline and a `flush`, all three answered for. When it fails there is no third stream to say so
  on, so `main::refuse` answers with **the status the run had already determined**: `2>/dev/null`
  and `2>/dev/full` are two ways of throwing stderr away and a script has to get the same answer
  from both, and `OUTPUT_FAILED` keeps the case it was minted for (a lost **answer** on stdout).
- Six gates in `crates/gx-cli/tests/model_a_probes.rs` (twenty-six → **thirty-two**), one per
  finding, each red on the previous binary and green on this one.
- `probes/doubt/tests/declaration_writer_doubt.rs` D-6 counts the error stream as well. The
  predicate is not "`eprintln!` is banned" — an operator note may stay a macro — it is "an
  `eprintln!` that carries a problem object is an answer, and answers go through the type". R13's
  exclusion said "every use of it in this crate is a sentence beside an answer", which was false at
  two sites; the sentence is corrected in place rather than deleted.
- 43 §7.16 (three clauses: the guarantee is counted per stream; the predicate that refuses is the
  predicate that decides the exit; a durable record holds no generations), 44 §1.3/§1.4's v0.5-a
  stacked note, 44 §2.3's `LAYOUT_BLOCKED` row, `docs/LIMITS.md` v0.5-a.

### Not fixed

- **`req/244` M-06 is still open.** The same class this release named at `.gx/repair` — a file where
  a directory belongs — is still `INTERNAL` one directory down, at `.gx/ledger/journal.blobs/`,
  which the engine's blob store creates. Closing it from `Layout` would be a second opinion about a
  directory this crate does not own.
- **`tools/e2e.sh:1375`'s symlink check is still the one gate in that script that reads "empty =
  pass".** This lane's write scope in that file is its `MIN_`/`EXPECTED_` lines.
- **The receipt-digest mismatch boundary is still not measured** (`req/246` §9, carried), and
  `/v1/healthz`'s cost has not been re-measured for a fifth release.

## R13 — a report is delivered, not composed (2026-08-17, `req/38` §183 ruling 2, `req/244`)

### Fixed

- **The report was guaranteed as a value and not as a delivery.** R12 made
  `repair::repair_and_report` return `Outcome` rather than `Result<Outcome>` and 43 §7.14 (b),
  `docs/LIMITS.md` v0.4-y and `req/243` §0 all said "it wrote something and told you nothing is now
  a compile error". The value was always produced; `main.rs`'s `print_json` was a `println!`, and
  Rust's `print!` family does not return a `Result` — it **panics** on a write error. Three
  destinations reproduce it with no adversary: a reader that closes first (`| true`), a full
  destination (`> /dev/full`), a reader that takes one byte. Each ended at exit **101** — a number
  no table in this repository publishes — with a Rust panic string where 44 §1.3's problem object
  belongs, over a `.gx/VERSION` that had been written; and the next `gx repair` answered
  `meta_repaired: []`, `meta_repair_refused: null`. `req/244` H-01.
- **A project could be locked out of gx entirely.** With `.gx/config.toml` and
  `.gx/ledger/journal` both gone, `gx submit` refused `CONFIG_ABSENT`, `gx repair --yes` returned
  from `run_the_repair`'s journal-absent branch without ever reaching `repair_meta` (so
  `meta_repaired: []`, and the remedy contained the word "config" **zero** times), and the next
  `gx submit` refused the same way — permanently. On a project that had never recorded a commit
  both repair runs exited **0**, which 44 §1.2 defines as "this project can be written to". Two
  forms, three runs each, no variation. The root is that `Layout::create` answered one question
  ("is this the init road?") with two predicates four lines apart: `Layout::logged` for the
  journal, `Layout::established` for the settings. `req/244` H-02.
- **A power cut during a `gx wrap` commit left a project no repair could close.** 43 §7-3b's
  window — `ledger.append` durable, `Committed` record not — is 21.0 ms wide on this machine
  (`strace`: leaf at `+134.7 ms`, commit receipt at `+149.2 ms`, record at `+155.6 ms`). Closing it
  used to require **re-applying** the delta, because 42 §3.10's `postcondition_fingerprint` is a
  reading of the world and no journal record carries one; a `gx wrap` commit is applied through an
  MCP server, `gx repair` has none, `adapter.apply` refused, and that refusal was answered with
  `Aborted(ApplyFailed)` — a **terminal** record, which is the record 43 §7-2 makes a recovery stop
  at. Measured on `e309b8d`: 28 crashes inside the window, **28** projects with
  `ledger_agrees_after: false` and every writer verb refusing `LEDGER_DISAGREES` afterwards, while
  `gx receipt verify` answered `valid: true`, `inclusion: "verified"` for the leaf the journal did
  not witness. The remedy asserted "the two files are from different projects", which was false in
  every arm. `req/244` H-03.
- **The 9p skip never fired.** `chmod_decides_writes` measured `fixture.home`, which
  `support::secure_scratch` puts under `std::env::temp_dir()` — ext4 regardless of
  `CARGO_TARGET_TMPDIR` — while the arm's subject is the project. So the guard answered `true`
  everywhere and the read-only probes simply failed on 9p (24 passed / 2 failed, neither printing
  `SKIPPED`). `req/244` M-01.
- **The write-road census had five blind spots**, one of which was a spelling this workspace
  already uses: `fs::copy(` (`gx-engine/src/store.rs`, `gx-log/src/store.rs`). Also
  `File::options()`, a path built more than three lines from the call, a `//` inside a string
  literal cutting the line before the call, and `Engine::open`/`open_reading`/`open_declared` —
  which pass `JournalCreation::Permitted` themselves, so one call from `gx-cli` would restore
  journal creation with neither string the census greps for. `req/244` M-02.
- **`DeclarationWriter::ensure_journal` did not read the declaration.** `create_journal` writes
  `GXJRNL01` unconditionally, so a project declaring `journal_format=legacy` got a **chained**
  journal from one `gx submit` at rc 0, after which `gx repair` answered
  `journal_format_declared: "legacy"`, `journal_intact: true`, `downgraded: false`, `remedy: null`.
  `req/244` M-03.
- **A project that had lost all three commit witnesses was handed a second history.**
  `Layout::logged`'s witnesses (`ledger/journal.ledger`, `checkpoints/head.json`, an entry under
  `receipts/`) all gone looks byte-for-byte like `gx key gen` in a fresh directory, so `gx submit`
  wrote a new journal over it and every later report said `journal_commits: 0`,
  `head_authenticity: "absent"`, `remedy: null`. `req/244` M-04.
- **A named `--signing-key` that would not resolve threw the report away** (rc 6 `NOT_FOUND` or
  rc 1, **zero** bytes on stdout, nothing written). `req/244` M-05.
- **A refused `gx submit` created `.gx/ledger/` on its way out** — absent before the run, present
  after it, from two `create_dir_all` calls the census does not count as writes. `req/244` L-04.
- **`kept_aside` counted names gx never writes**: R12 closed the tail (`.txt`, `.9`) and left the
  stem open, so `.gx/notes.pre-repair.3` was still reported as a file gx had set aside.
  `req/244` L-01.
- **`print_json` printed an empty line at exit 0 when the serialiser refused**
  (`text.unwrap_or_default()`). No reachable input produces it today, which is not the same fact as
  "there is no road". `req/244` L-03.

### Changed

- **`crates/gx-cli/src/emit.rs` is new: the one road to stdout.** `Outcome::emit(&mut dyn Write,
  pretty) -> io::Result<()>` delivers the JSON object 44 §1.3 gives each verb — `write_all`, a
  newline, and a **flush**, every failure a value — and `emit::line` / the `say!` macro carry the
  plain-text output of `gx limits`, `gx demo` and `gx serve`'s start-up line. A failed delivery is
  exit **1** and an `OUTPUT_FAILED` problem object on stderr.
  `probes/doubt/tests/declaration_writer_doubt.rs` **D-6** counts `println!`/`print!` call sites in
  `crates/gx-cli/src/` and requires **zero** outside `emit`.
- **`gx repair --yes` files its own report at `.gx/repair/last.json`**, which the next `gx repair`
  prints back under `previous_repair`. Declared as req/56 §2's eleventh row and `GX_PATHS`'
  eleventh (`Nature::Source`, `Shape::Dir`) and deliberately **outside** `Layout::logged` and
  `Layout::established`. A record that cannot be written does not raise; it appears as
  `repair_record.written: false`.
- **`Layout::create` uses `Layout::logged` for both `Nature::Meta` files.** A directory that has
  never recorded a commit gets its `config.toml` written on the init road, as it already got its
  journal; a project that **has** committed keeps `CONFIG_ABSENT` (43 §7.9 (b)'s R9 row — this file
  decides the recovery key) and its exit is `gx repair --yes`, which now repairs the settings on
  the journal-absent road (`MetaScope::SettingsOnly`) because their bytes are the shipped default
  and ask the journal nothing.
- **43 §7-3b closes without asking an adapter anything.** Two new `RecoveryPath` values:
  `ClosedFromFiledReceipt` (the commit receipt the critical section already filed digests to the
  leaf the ledger witnessed, so the record is written from the document) and `ClosedFromLedgerLeaf`
  (no receipt was filed and the substrate cannot be read, so the record is written from the leaf
  and **no receipt is issued**). A failed re-apply on a row whose leaf the ledger holds no longer
  writes a terminal record. Measured on the same sweep: **48 of 48** in the window closed,
  `journal_behind_by: 0`, the next `gx submit` rc 0 (10 from the receipt, 38 from the leaf).
- **`gx repair`'s report gains four keys**: `previous_repair`, `repair_record`,
  `journal_behind_by`, and `recover.closed_from_receipt` / `recover.closed_from_leaf`. The three
  report shapes still carry one key set in one order.
- **`OUTPUT_FAILED` and `HISTORY_LOST`** join `gx_code.rs`'s `RULED_ADDITIONS` (ten → **twelve**,
  both 500 / CLI 1) and `sdk/typescript`'s `GX_CODES` (twenty-two → **twenty-four**).
- **`JournalCreation`'s `Default` is `Refused`** (`req/244` L-07). The doors that mean "create it"
  — `EngineJournal::open_declared`, `Engine::open` — pass `Permitted` **by name**, so an embedder
  calling either sees no change; what changes is what `..Default::default()` and
  `ProjectAnchor::none()` mean, and they now fail safe. An embedder that built a `ProjectAnchor`
  with `..Default::default()` and relied on the engine to create a journal writes
  `journal_creation: JournalCreation::Permitted`.
- **Test floor 1770 → 1775 over 318 suites** (D-6 and D-7 in an existing suite). Ten
  self-adversarial gates were run against `e309b8d`'s binary and against this one: **ten red
  before, ten green after**.

## R12 — the declaration has one writer, and it is a type (2026-08-17, `req/38` §181 ruling 2, `req/242`)

### Fixed

- **`.gx/VERSION` was written by three roads, not one.** `session::anchor_accepting` →
  `Layout::declare_journal_format` rewrote the declaration on the writer's road — `gx submit`,
  `gx plan`, `gx commit`, `gx undo` and `gx serve`'s start-up — whenever the file carried no
  `journal_format` line. It never appeared in `meta_repaired`, took no `VERSION.pre-repair.<n>`
  copy, and its gate ("`declaration_lines` returned a non-empty bare line") was not the gate every
  door refuses on ("the first bare line is a number"). A `.gx/VERSION` holding `1.0`, `x` or three
  NUL bytes was `DECLARATION_UNREADABLE` at every door and rewritten by the next `gx submit`
  (3 bytes → 27, 50 → 74, three runs each). Deleting the `journal_format` line from a healthy
  project made `gx repair` raise R7's `rolled_back`; one `gx submit` (rc 0, empty stderr) put the
  file back and returned `head_authenticity: verified`. `req/242` H-01.
- **`gx repair --yes` wrote `.gx/VERSION` and `.gx/config.toml` and then threw the report away.**
  R11 moved the write below the lock and the key; the report is composed after the engine opens,
  catches up and recovers, and three `?` sat in between. Four kinds of damage under `.gx/ledger/`,
  twelve runs, no difference: rc 1 `INTERNAL`, **zero bytes** on stdout, 25 bytes of declaration on
  the disk. `req/242` H-02.
- **A `.gx/ledger/journal` that was gone was re-created, empty, by the next writer**, after which
  the report about the loss said `journal_absent: false`. `req/242` H-01 (d).
- **`gx repair --yes` answered `INTERNAL` with an empty stdout on a tree where `.gx/LOCK` cannot be
  created** (a backup, a `git archive`, a read-only snapshot — the file is `Nature::Transient` and
  gx does not ship it). `req/242` M-03.
- **`gx repair --yes` said nothing at all about a declaration whose version line is not a number**:
  `meta_repaired: []` and `meta_repair_refused: null` on the same run. `req/242` L-03.
- **The SDK's `gx_code` census read `dist/`**, a git-ignored build output, so it compared the
  server's source against an artifact. `req/242` M-04.

### Changed

- **`crates/gx-cli/src/declaration.rs` is new**: `DeclarationWriter` owns every `std::fs` call that
  writes `.gx/VERSION`, `.gx/config.toml` or a new `.gx/ledger/journal` — **five**, all private —
  and the two functions that compose those bytes (`declared_text`, `default_contents`) are private
  to it. Two constructors: `for_init` (a directory that is not a project yet) and `for_repair`,
  which takes `&OwnedLock` and `&KeyPair` **by reference**, so `req/240` H-01's ordering is the
  signature rather than the order of two statements. `Layout` is the read-only handle everything
  else holds and has no method that writes any of the three.
- **`Layout::declare_journal_format` is removed.** Nothing stamps a framing on to an existing
  project any more. A project this binary creates declares `journal_format=chained` in the same
  call that creates it and its journal; a project that never declared one keeps the pre-R6
  treatment (`declared_format: None`, reported as `journal_format_declared: null`). Its own doc
  comment's claim of "a one-shot window per project" was measured false — the window reopened every
  time the line was deleted.
- **`repair::repair_and_report` returns `Outcome`, not `Result<Outcome>`.** After the lock and the
  key are in hand there is no way out of `gx repair` that does not print. The engine's open,
  `catch_up`, `recover`, `accept_rollback` and receipt re-issue are values; a run that could not
  open the engine prints the same key set with `engine_open_failed: {stage, reason}`.
- **`gx_engine::JournalCreation`** (`Permitted` by default for library callers, `Refused` on every
  road `gx-cli` takes) decides whether `EngineJournal::open_declared_creating` may bring the file
  into existence. `gx-cli` builds `ProjectAnchor` in one place and always refuses.
- **`Layout::recover`'s `Nature::Meta` arm reports instead of writing** (`Recovery::Lost`, not
  `Recovery::Initialised`) — a fourth write road no verb reaches and no audit had found.
- **`JOURNAL_ABSENT`** is 44 §2.3's tenth ruled addition (500 / CLI 1), and the SDK's `GX_CODES`
  is **22** words. The SDK census reads `src/errors.ts`; a present `dist/` is checked against it.
- **`tools/e2e.sh` gains stage 5b**: `sdk/wasm-verify/build.sh`, `npm ci`, `npm test` from the
  clone. `MIN_SDK_PASS=18` / `MAX_SDK_SKIP=7` are their own line — `MIN_PROBES` counts what cargo
  printed. The stage reaches the network (`npm ci`), which is declared in the script's own gaps.
- **Floor: 1758 → 1768 probes over 317 → 318 suites.** `model_a_probes.rs` 20 → 25 (five gates,
  all five red on `0dde16b`); `probes/doubt/tests/declaration_writer_doubt.rs` is the new suite —
  five probes that count the write roads from the source, all five red on `0dde16b`.
- **Read-only probes measure the filesystem first** (`chmod_decides_writes`): on WSL's 9p a `555`
  directory stays writable, so the arm skips and prints why instead of going red. `req/242` M-06.
- **`.pre-repair` refusal no longer calls `.0` "the oldest"** — moving one out of `.gx/` frees the
  number and the next repair reuses it. `req/242` L-01.


## 0. What this file is for — two clauses name it directly (sem: SEM-CHANGELOG.md-002)

| Clause | Verbatim | This file's role | (sem: SEM-CHANGELOG.md-003)
|---|---|---|
| 33 **NFR-024** | "the semver policy during pre-1.0: a minor bump between `0.y.z` may carry a breaking change, but **it must be recorded in the CHANGELOG**. From 1.0 onward the project moves to strict semver" | A breaking change is not forbidden. **Changing it silently** is forbidden. This is where that record lives | (sem: SEM-CHANGELOG.md-004)
| 47 **§4** | "**the journal schema makes it a pre-upgrade verification condition that `gx replay`'s deterministic replay agrees between the old and new binary**" | If 42 §3.13's `EngineJournalRecord` shape changes, an existing journal file stops reading. This is where a reader learns **which window changed the shape** | (sem: SEM-CHANGELOG.md-005)

🔴 **While this file did not exist, 47 §4's condition sat in the state "declared, but with nothing that enforces it"** (`req/110` §1 NFR-024, §2-④-g "CHANGELOG.md absent"). This file fills that gap, but **it does not, by itself, make a gate** -- "put a CHANGELOG-required-items checklist in the release PR template and enforce it in review" (NFR-024's measurement-method column) does not exist yet. **A vessel existing and a machine standing watch over it are two different things.** (sem: SEM-CHANGELOG.md-006)

## 1. 🔴 Honestly, first — what is written here is not a release (sem: SEM-CHANGELOG.md-007)

- **Nothing has been published to crates.io.** Every crate in the workspace still reads `version = "0.1.0"`, and `gx-substrate-conformance` and `probes/doubt` declare themselves not-for-publish in their own manifests. (sem: SEM-CHANGELOG.md-008)
- **There is no signed release artefact either** (47 §1(b); 33 NFR-013's SLSA provenance has not been started). (sem: SEM-CHANGELOG.md-009)
- ∴ **the table's `req0.0x` values below are not "public releases" -- they are internal markers struck the moment the requirement lane (`req/38`) issued a PASS acceptance**. The values were taken mechanically from `git tag -l` and each tag's date (`git tag -l --sort=creatordate --format='%(refname:short)|%(creatordate:short)'`). The summary sentences are transcriptions of each tag's subject, not an evaluation this file added. (sem: SEM-CHANGELOG.md-010)
- **The first row that carries semver meaning has not been written yet.** The same is true of the move to strict semver from 1.0 onward (NFR-024). (sem: SEM-CHANGELOG.md-011)

## 2. Unreleased

- 🔴 **R11 — the eleventh adversarial audit's Model A repairs** (2026-08-17, `req/38` §179 ruling 2, `req/240`, 43 §7.9 (b)/§7.13, 44 §2.3 v0.4-x, `docs/LIMITS.md` v0.4-x). **The journal schema (`EngineJournalRecord`) has not moved a single byte**, and no receipt, checkpoint or head payload changed — every document a previous release signed still verifies, and every existing journal file reads exactly as it did.
  - **Behavioural change, recorded here because NFR-024 requires it: `gx repair --yes` writes a `Nature::Meta` file only after it has taken the project lock and resolved a signing key.** R10 put that write at the top of the verb, above both. `req/240` H-01 measured what it cost on the road a buyer actually walks: a project gx creates records no `engine_signing_keyid`, so the `gx repair --yes` that `DECLARATION_ABSENT`'s own remedy names **always** reached the key check, exited **1** `VALIDATION_ERROR` with an **empty stdout** — and had already written `.gx/VERSION` and `.gx/config.toml` back (five arms, byte-identical). With an operator's own settings line in the declaration, that silent write put a **different** declaration back, the head's recorded digest never matched again, and the second `--yes` (with a key) answered `meta_repaired: []` — no trace that the file had been missing or that gx had rewritten it. A foreign lock and a read-only `.gx/` produced the same silence.
    - Order is now **lock → key → meta repair → engine → recover → report**. A `BUSY` refusal writes nothing; a run that cannot resolve a key, or cannot write, **degrades to a report**: it measures everything a report measures, prints the whole diagnosis on stdout, says what it could not do and how to fix that in the new `meta_repair_refused` key, and exits 1. **This verb no longer has an exit-1-with-empty-stdout path.** That also closes audit 10 M-03 (early errors discarding the report) and `req/240` M-06 (a read-only `.gx/` answering `INTERNAL`, 44 §2.3's word for "not classifiable", about a fact that is entirely classifiable).
  - **Behavioural change: `gx repair` no longer answers exit 0 about a project whose `.gx/ledger/journal` is gone.** R4's early return printed `ledger_agrees_before: true`, `journal_commits: 0`, `ledger_leaves: 0`, `remedy: null` as **constants**; `req/240` H-02 measured them over a project with two committed leaves, two receipts and a signed head — refused `LEDGER_DISAGREES` by the very next `gx submit`. The branch now reports the **same forty-seven keys** as every other report, measured off the files that are still there (the ledger through `LedgerStore::open_read_only`, the head through `HeadStore::read`, the receipts off the archive) with `null` — not `0`, not `false` — for what needs the engine. Exit **1** with a remedy when the project holds leaves, receipts or a head; exit **0** when it has never held a commit. `--yes` writes nothing here either: the framing it would record is sniffed off the journal's first eight bytes, and gx does not guess it. New key: **`journal_absent`**.
  - **Behavioural change: `gx serve` asks about `.gx/VERSION` and `.gx/config.toml` at every write, not once at start-up** (`req/240` M-04). With a server up, `rm .gx/VERSION` left the wire unchanged — `/v1/healthz` `{"status":"ok"}`, `POST /v1/candidates` 201, verify 200, commit 200, a leaf added — while every CLI verb refused the same project. Writes now refuse with the same two words (`DECLARATION_ABSENT` / `CONFIG_ABSENT`, 500), and **`GET /v1/healthz` gains a fifth member**: `status` may be `"degraded"` and `status_reason` (`string | null`) names the file and the way out. The status code stays **200** — reads still work; `500 LEDGER_DISAGREES` remains the word for a ledger that cannot be trusted (44 §2.3 v0.4-x).
  - **`GET /v1/healthz` answers from a snapshot taken outside the engine's `Mutex`** (`req/240` M-01). The reuse is **witnessed**, not merely timed: `ProjectMeta::witness` stats the journal and the ledger (two syscalls, no lock, no read) and the snapshot is only reused while that is unchanged, so every rewrite under a running server still reaches the **next** probe. **The first attempt at this was a plain 250 ms timer and it turned five existing detector probes red at once** (`serve_runtime_r2`'s `a_ledger_that_moved_makes_healthz_say_so_by_name`, `r3`'s and `r4`'s same-length-rewrite arms, `r4`'s shrunk-journal arm) — 43 §7.12 (c)'s refusal to cache this endpoint was right, and those probes are where it is written down as a machine. None of them was weakened. `HEALTH_SNAPSHOT_MAX_AGE` = **250 ms** is now a ceiling on reuse, covering only a change that leaves both files the same length with the same mtime, and a write through this server drops the snapshot as its guard falls. **Measured, honestly** (400 commits, before = `3feb35e`, two paired runs each): the probe itself is consistently cheaper (sequential median 2.11 / 2.06 ms → **1.40 / 1.17 ms**); the write-under-load arms moved less between binaries than between two runs of the same one (fixed 198 probes/s: 53.5 / 55.7 ms before, 103.5 / 34.5 ms after), so **no claim is made about them**. The audit's own absolute numbers (8.81 ms sequential, 125 req/s) were not reproduced on this lane's harness, so nothing is claimed about its ×4.1 either.
  - **`Layout::create` no longer writes `.gx/.gitignore` into an established project** (`req/240` M-02) — req/56 §4 invites the operator to edit that file, and deleting it used to bring the shipped default back at exit 0 in silence. Reported instead, as `gitignore_absent`. **It is deliberately not a `GX_PATHS` row**: that would move req/56 §2's table and `m6_surface_doubt.rs`'s three-way check, and req/56 is outside this lane's write scope (43 §7.9 (b) carries the row).
  - **`*.pre-repair.<n>` is a declared family with a ceiling** (`req/240` M-03): 43 §7.9 (b) has a row, `gx repair` publishes **`kept_aside`**, and `gx_cli::layout::PRE_REPAIR_LIMIT` = **8**. Past it a repair **stops and names the oldest copy** rather than making a ninth; nothing removes one (they are the bytes that were in the file). R10's `0..1000` had no listing, no ceiling worth the name, and turned `gx repair --yes` itself into a `Usage` refusal at the thousandth.
  - **Carried repairs**: a `.gx/VERSION` declaring a newer layout version now carries a remedy in the refusal itself (audit 10 M-04 — the door stays shut, 47 §4); `gx repair --yes` sweeps only the `.tmp` names gx writes (`head.json.tmp`, `<id>.commit.json.tmp`, `<id>.verdict.json.tmp`) and reports anything else without touching it (audit 10 L-01); `BlobStore::put`'s cost paragraph is corrected — on the fs substrate the *n*-th commit's inverse is the *(n−1)*-th commit's delta, so the `AlreadyPresent` byte comparison is every commit's road, not a rare re-put (audit 10 M-05); `<cid>.blob.tmp.<pid>`'s doc says the safety is DR-43-2's lock and not the pid (audit 10 L-02); the missing-inverse remedy no longer asserts a third party took it (audit 10 L-03/L-04).
  - **SDK** (`req/240` L-06, audit 9 L-02 — `sdk/` was in scope this time): `GX_CODES` names all **twenty-one** codes on the wire (44 §2.3's twelve plus `gx_code.rs`'s nine `RULED_ADDITIONS`; it held thirteen and said so in its own prose), `inverse_status` is typed as 42 §3.12's six values instead of `unknown`, and `ProblemDetail` carries `retry_after_ms?`. `sdk/typescript/test/gx_code_census.test.mjs` reads the Rust source and fails when the two disagree.
  - **`gx_code.rs`'s "the CLI never meets them" is struck as false** (`req/240` L-07): until this release those two words were ones **only** the CLI produced. The exit stays **1** for the reason that stands on its own — 44 §1.4 has no exit for either.
  - **Probe denominator**: `crates/gx-cli/tests/model_a_probes.rs` 15 → **20**, all five new ones attacks on this release's own new code, and **all six of the corresponding gates measured red on the previous binary** (`3feb35e`): the keyless `--yes` wrote `.gx/VERSION` with an empty stdout, the read-only arm printed no report, the journal-less project answered exit 0 with 13 keys and `ledger_leaves: 0` over two leaves, `.gx/.gitignore` came back, `kept_aside` did not exist, and the serving process answered 201 and `"ok"`. `three_processes_writing_at_once…` now asserts that every non-zero CLI exit carries `gx_code == "BUSY"` (`req/240` §3-3 — `BUSY`, `LEDGER_DISAGREES`, `DECLARATION_ABSENT` and `INTERNAL` all exit 1, so counting non-zero exits as "the lock fired" would let an engine race pass for the exclusion working).

- 🔴 **R10 — the tenth adversarial audit's Model A repairs** (2026-08-17, `req/38` §177 ruling 2, `req/238`, 43 §7.9 (b)/§7.12, 44 §2.3 v0.4-w, `docs/LIMITS.md` v0.4-w). **The journal schema (`EngineJournalRecord`) has not moved a single byte** — no record was added, removed or reshaped, so 47 §4's replay condition is untouched and every existing journal file reads exactly as it did. **No receipt, checkpoint or head payload changed either**, so every document a previous release signed still verifies.
  - **Behavioural change, recorded here because NFR-024 requires it: no verb creates `.gx/VERSION` or `.gx/config.toml` in a project that already has a journal.** `Layout::create` was idempotent by writing the defaults for any `Nature::Meta` file it found missing, and `Layout::declare_journal_format` read the declaration with `read_to_string(..).unwrap_or_else(|_| "1\n")` and `raw.lines()` — the one reader of that file in the workspace that did not go through `gx_log::head::declaration_lines`. `req/238` H-01 measured the consequence with no adversary and no unusual step: delete `.gx/VERSION`, and `gx repair` exits **6** with **no report at all**; run anything that writes (`gx submit` is enough) and the file is recreated from defaults at exit **0**, in silence, taking `ledger_agrees_before` from `false` back to `true`, `head_authenticity` back to `verified` and `remedy` back to `null`. R7 bound that file's digest under the head's signature so a *rewritten* declaration would be caught; **deleting it was the stronger attack**, because the next writer erased the evidence with the fault. Two more faces of the same hole: a declaration that is **not text** had the operator's bytes discarded and replaced at exit 0, and a deleted `.gx/config.toml` came back as the shipped default, so `engine_signing_keyid` — the setting 43 §7.9 (b) calls the one that decides the recovery key — silently stopped being set.
    - `Layout::create` now writes those defaults only for a directory that is **not yet a project** (no `VERSION`, no `ledger/journal`, no `checkpoints/head.json`): the `gx submit` init road, `gx demo`, a test fixture. **A fixture that builds a journal before calling `Layout::create` now fails**, and the order every road in the binary uses is layout-then-engine (`crates/gx-cli/tests/ac_055.rs` was the one place with it reversed).
    - `Layout::declare_journal_format` goes through `declaration_lines`, and the text it writes is composed from what the declaration **says** rather than from the bytes it happened to hold.
    - `gx repair`'s report mode opens on both absences and writes nothing; `gx repair --yes` is the only road that writes either file back, and it names what it did. An unreadable declaration it rewrites is moved to `VERSION.pre-repair.<n>` first and kept. It never invents the `engine_signing_keyid` line.
  - **Additive vocabulary: `gx_code = DECLARATION_ABSENT` and `gx_code = CONFIG_ABSENT`** (44 §2.3's ruled additions v0.4-w, 500 on the wire / **1** on the command line — no new exit number, `req/38` §148's standing rule). Kept apart from `DECLARATION_UNREADABLE` (which is "present and does not read", with a different remedy) and from `NOT_FOUND` (44 §1.4's 6, which is "the object you named is not here" — nobody named this file). **A directory with no `.gx/` at all still exits 6**, so a mistyped path is not reported as damage.
  - **`GET /v1/healthz` no longer rebuilds Σ** (`req/238` M-06). The one endpoint 44 §2.5 keeps outside the bearer guard called `engine.sigma().ledger().len()` on every request — four vectors allocated, every state row copied, all four sorted — to answer one number, and the cost grew with the project (measured, median of 30: **1.52 ms** at 5 commits, **11.92 ms** at 400). `Engine::committed_len()` is the same number in O(1). **This does not make the endpoint O(1), and the residual was attributed rather than assumed**: after the change, 1.58 ms at 5 / 3.47 ms at 100 / **8.93 ms** at 400, and two throwaway binaries put 0.4 ms of the remainder on `ledger_agrees()` and the rest on `AppState::engine_refreshed`'s lockless catch-up — R4's detector for a journal or ledger rewritten under a running server (skipping it: **1.66 ms** at 400, flat). Not removed and not cached; declared in 43 §7.12 (e) (A12).
  - **New keys on `gx repair`'s stdout**: `declaration_absent`, `config_absent`, `meta_repaired` (`req/238` H-01), `head_behind_by` (audit 8 M-02 — a number instead of parsing the `rolled_back` sentence) and `journal_intact_basis` (audit 8 M-05 — `chain` / `length-only` / `not-intact`, because `journal_intact: true` means a weaker thing for a legacy journal than for a chained one). Nothing was removed.
  - **New verb `gx draft list`** (audit 8 M-06 + L-02): the drafts the journal witnesses, each saying whether `.gx/drafts/` still holds its body, with the body-less ones counted as `bodyless`. A read — it does not take the project lock.
  - **`gx log checkpoint --key` accepts a key id** as well as a file path (audit 8 L-01; a key id used to arrive as `INTERNAL` "stat the key …"). **`gx receipt verify` accepts a `gx1:` transformation id** as well as a path (`req/222` L-04); the file road is unchanged and still tried first, because AC-057's third party has a document and no project.
  - **`gx receipt verify` always publishes `issued_at_signed: false`** and `issued_at_unix_nanos` (audit 9 L-03). **E-M2-6** put `issued_at` outside the signed core deliberately and that ruling is not reversed here — what changes is that the answer says so instead of leaving the fact in four source files.
  - **The `BUSY` message no longer states who holds the lock as though it knew** (audit 9 L-01). The note inside `.gx/LOCK` is written by whichever process took it and is never re-read, and it was measured naming a `gx` that had already exited while a non-gx process held the flock.
  - **Not done, stated**: `sdk/typescript`'s `inverse_status: unknown | null` (audit 9 L-02) — outside this lane's write scope. The fix is one line: narrow it to the five `InverseStatus` names and regenerate `dist/types.d.ts`.
  - Test floor 1750 → **1753** probes over 317 suites (`crates/gx-cli/tests/model_a_probes.rs` gains three probes, all three attacks on this release's own new code — a declaration that is gone, settings that are gone, and a declaration that is not text; no new suite). The whole suite was also run once with its fixture root on WSL's 9p mount (`v9fs`, confirmed with `stat -f`): **15/15**, with the binary and the key store still on ext4.

- 🔴 **R9 — the ninth adversarial audit's Model A repairs** (2026-08-17, `req/38` §175 ruling 2, `req/236`, 43 §7.11, 44 §1.2/§2.3 v0.4-v). **The journal schema (`EngineJournalRecord`) has not moved a single byte** — no record was added, removed or reshaped, so 47 §4's replay condition is untouched and every existing journal file reads exactly as it did.
  - **Behavioural change, recorded here because NFR-024 requires it: a body already filed under a content address is only reused if its *bytes* match.** M4H6-3's "if the CID is the same, register reference-only" was implemented as `if path.exists()`, and `BlobStore::put` had no temporary file, no rename and no cleanup. `req/236` H-01 measured the consequence with no adversary: a full disk left **204,800 bytes of a 400,096-byte inverse at its own content address**, permanently, and the next entirely successful commit (`rc=0`, signed receipt, `gx receipt verify` clean) adopted the fragment as its own escrowed inverse — after which `gx repair` reported the project healthy, the API reported `inverse_status: "Available"`, and `gx undo` failed for ever with `INTERNAL`. The write is now tmp + `fsync` + `rename` + directory `fsync`, a re-put compares the bytes, `Engine::commit` checks that the inverse it escrowed reads back **before** `ApplyStarted`, and `Available` means the body was read, decoded and matched its own name. `crates/gx-engine/tests/blob_store.rs::a_second_put_of_a_known_cid_writes_nothing` carries the struck claim it replaced.
  - **Behavioural change: a crash recovery run under the wrong signing key no longer writes a terminal record.** 43 §7-3b rebuilds the commit receipt's payload and compares its digest with the leaf; `key_id` is a field of that payload, so a recovery under any other key could never match — and the mismatch arm wrote `Aborted(InternalError)`, which is terminal, so one run under the wrong key removed the row's only way out permanently (`req/236` H-03: the committing key 7 runs 0 bricked, another key **8/8 bricked**, `gx serve --signing-key <other>` **7/7 bricked**). The payload is now rebuilt under the key the **already-filed receipt** names, and where no receipt has been filed yet the recovery refuses with `RecoveryPath::NotResumed` and leaves the row resumable. `Recovered::receipt` is now `None` when the archive already holds the issued document (re-signing it under another key would mint a receipt whose `key_id` and signature disagree).
  - **`.gx/VERSION` is decoded before it is parsed** (`req/236` H-04). A UTF-8 byte-order mark, a leading blank line, bare-CR endings, a UTF-16 LE save, or the two lines swapped each stopped `gx repair` (report **and** `--yes`), `gx log proof`, `gx replay` and `gx serve` with `VALIDATION_ERROR` and no remedy. `gx_log::head::declaration_lines` is now the single reader behind both the parse and the digest, the layout version is "the first line that is not a `key=value`" (so line order carries no meaning), and `normalise_declaration` stable-sorts settings by key — duplicate keys keep the file's order, so "the first one wins" still means the same thing on both sides. **Not a compatibility break**: `Layout::declare_journal_format` writes the normal form byte for byte, so a head recorded by an R7 or R8 binary beside a gx-written `.gx/VERSION` digests to the same value.
  - **Additive vocabulary: `gx_code = DECLARATION_UNREADABLE`** (44 §2.3's ruled additions, 500 on the wire / **1** on the command line — no new exit number, `req/38` §148's standing rule). For a `.gx/VERSION` that is present and still does not read: bytes that are not text, or a file with no layout-version line. It carries the shape the bytes are in and the two correct lines, and **`gx repair` opens anyway** and reports everything else (`Layout::open_reporting`).
  - **`gx undo` (CLI) answers `INVERSE_UNAVAILABLE` for a `BodyMissing` row** (`req/236` M-01), which is the word the HTTP face has always used; it used to reach the blob store and exit with `INTERNAL`. **The settle pre-flight answers `BUSY`** when it cannot retake the project lock after waiting (`req/236` M-02) — it used to answer `PRECONDITION_CHANGED` and tell the operator to restore an undamaged receipt.
  - **Five new keys on `gx repair`'s stdout** (`damaged_bodies`, `damaged_body_names`, `staging_files`, `staging_files_swept`, `declaration_readable`) and one inside `recover` (`payload_mismatch`). `--reissue-receipts` gains the refusal reason `key_mismatch` (`req/236` M-05: every row of an unmoved project was reported as `world_moved` when the key was wrong). The `gx_code` table's twelve rows and the exit-code table are unchanged.
  - **`gx commit` writes its commit receipt once.** R8 registered the engine-side sink and left the CLI's own write in place, so the same path was written twice per commit (measured with `strace`: two create+`fsync`+`rename`+directory-`fsync` pairs). `req/235` §7-4's "the number of writes has not gone up" was false and is corrected here rather than edited there. `ReceiptStore::put` now writes nothing when the bytes on disk already match.
  - **Corrected in `docs/LIMITS.md` v0.4-v (no-delete, the v0.4-u text stands)**: "it now reads the store before answering and says `BodyMissing`" (the read was of the *name*) and "`gx repair` counts them" (the count was structurally always zero, because `Engine::sigma()`'s escrow component came from a live table that `Engine::open` leaves empty — `req/236` H-02). `Engine::sigma()` now reads live ∪ Σ-shadow, which is the same rule `Engine::inverse_status` already used.
  - Test floor 1746 → **1750** probes over 317 suites (`crates/gx-cli/tests/model_a_probes.rs` gains four probes, three of them attacks on this release's own new code; no new suite).

- 🔴 **R8 — the eighth adversarial audit's Model A repairs** (2026-08-17, `req/38` §173 ruling 2, `req/234`, 43 §7.10, 44 §1.2 v0.4-u). **The journal schema (`EngineJournalRecord`) has not moved a single byte** — no record was added, removed or reshaped, so 47 §4's replay condition is untouched and every existing journal file reads exactly as it did.
  - **Behavioural change, recorded here because NFR-024 requires it: a commit whose receipt cannot be filed now fails.** Through R7 the receipt archive was written by the caller *after* `Engine::commit` returned; a failure there was reported (HTTP `500`) while the row stayed `Committed`. Since R8 the archive write happens inside T-11's critical section and **in front of** the `Committed` record, so the same failure leaves the row in `Committing` with its leaf on the ledger — 43 §7-3b's own window, closed by the next start-up. `req/38` §154 is the rule; `req/234` H-01 is the measurement (a power cut in the old window lost the receipt permanently, for 44% of a commit).
  - **Additive vocabulary: `InverseStatus::BodyMissing`** (`req/234` B-5). The sixth value, after 42 §3.12's four and `Pending`. It is **never written to disk** — `Engine::inverse_status` reads the blob store before answering, so an escrow row whose body is gone from `.gx/ledger/journal.blobs/` stops being reported as `Available`. A consumer matching exhaustively on the enum gains one arm; `POST /v1/transformations/{id}/undo` answers `409 INVERSE_UNAVAILABLE` for it where it used to reach the engine's `INTERNAL`.
  - **`.gx/VERSION`'s digest is taken over the declaration rather than the bytes** (`req/234` H-02, `gx_log::head::normalise_declaration`). **Not a compatibility break**: `Layout::declare_journal_format` writes `lines.join("\n") + "\n"`, which is byte-identical to the normal form, so a head recorded by an R7 binary beside a gx-written `.gx/VERSION` digests to the same value. What changes is which *other* byte strings now agree with it — a trailing newline, CRLF, or a trailing space no longer stop a project from starting.
  - **New flag `gx repair --yes --reissue-receipts`** and seven new keys on `gx repair`'s stdout (`commit_receipts`, `receipts_missing`, `receipts_missing_ids`, `reissued`, `escrow_bodies_missing`, `escrow_bodies_missing_ids`, `files_agree`). The `gx_code` table and the exit-code table are unchanged.
  - **`gx undo --settle` no longer holds `.gx/LOCK` while it waits** (`req/234` H-03). The flag's meaning and default (120 s) are unchanged; what changes is that the rest of the project is not `BUSY` for the duration. New cost, stated: an undo can now end in `BUSY` if another `gx` takes the lock during the wait.
  - **Declared and not closed**: the gap between commit's compare and the adapter's write. Measured with `strace` on ext4: **23.5 ms → 14.3 ms**, of which ~13 ms is two `fsync` calls. 43 §7.10 (b) carries the number and **DR-43-13** carries the design question.
  - Test floor 1743 → **1746** probes over 317 suites (`crates/gx-cli/tests/model_a_probes.rs` gains three probes; no new suite).

- **v0.2.4 spec batch** (2026-08-13, `req/38` §68, 10 assigned items): synced 42 §3.13's remaining 6 record differences to reality (doc-only -- **the journal schema's shape has not moved a millimetre** -- what moved was the canon side; `crates/gx-engine/src/store.rs`'s `EngineJournalRecord` has not changed a single byte) / corrected 42 §0's type attribution / filled out 41 §2's crate tree / resolved 33 NFR-018's placeholder shape / defined 47 §2's T1-T4 / transcribed 35 §E/§F's rulings / created this file. (sem: SEM-CHANGELOG.md-012)
  - 🔴 **Read this as two different things**: "42 §3.13 changed" means **the canon caught up with the implementation**, and **journal file compatibility was never once affected**. What 47 §4 makes the condition is the implementation's schema, not the canon's spelling. (sem: SEM-CHANGELOG.md-013)
- **M9 P1** (2026-08-13, `req/115` §A, `req/128`): added a third `SubstrateAdapter` (`gx-adapter-postgres`, workspace members 15→16). `SubstrateKind::Custom("postgres")` (42 §3.1's existing escape hatch; the type is unchanged). Row operations only, for 3 single-statement DML kinds (INSERT/UPDATE/DELETE), a single table, a single-column PK (out of scope: DDL, multi-statement, no PK, or multi-row -- these fail closed with a named refusal at `plan`/`snapshot`). Two new dependencies: `sqlparser` (Apache-2.0), `postgres` (MIT OR Apache-2.0); primary verification is `Desktop/GitRepo/REFERENCES.md` 2026-08-13. **The journal schema (`EngineJournalRecord`) has not moved a single byte** (out of §3's scope -- an adapter-layer-only change). (sem: SEM-CHANGELOG.md-014)

- **v0.3-a A-2' two-phase escrow** (2026-08-15, `req/38` §98 ruling 1 + §99 ruling, `req/162`): journal vocabulary 13→15 (new `ApplyObserved`/`InverseCompleted`, neither a transition -- both are points inside T-9's critical window) / `InverseEscrowed` gains `pending: bool` (**serde default + `false` is omitted on the wire, so an old journal still reads, and the old record's canonical bytes are unchanged too** -- not the kind of change that adds a row to §3's table; 47 §4's replay agreement holds by construction) / `InverseStatus` gains `Pending` (42 §3.12, additive) / a new observation store (`<journal>.observations`, raw bytes, capped at 1 MiB) / catalogue `ArgSource` gains `do_result`/`do_result_number_from` (opt-in; behaviour changes only for the declared pair) / `ToolTransport::call` moves to returning result content bytes (an internal `gx-adapter-mcp` boundary; `SubstrateAdapter`'s 7 methods are untouched = N-08 unchanged; completion is a separate optional trait, `gx_substrate::InverseCompletion`). (sem: SEM-CHANGELOG.md-015)

- **v0.3-a A-3 settle pre-flight** (2026-08-15, `req/38` §98 ruling 2, `req/163`): added a settle pre-flight to `gx undo` (120s default, `--settle <SECS>`, 0 = disabled -- before firing, the CLI polls `Engine::live_digest` (a read-only probe, journal untouched) with the commit receipt's `postcondition_fingerprint` as the expected value; a timeout still fires once, unchanged result vocabulary) and `--retry <N>` (default 0; retries the whole thing only on exit 5 = `Aborted(ApplyFailed)`; each attempt is stacked into the journal as an independent `T_u`). **The journal schema (`EngineJournalRecord`) has not moved a single byte** (out of §3's scope -- pre-flight is read-only; the poll distribution goes to stderr). Resolves 44 §1.2's "settle/retry reserved for v0.2" clause, additively. (sem: SEM-CHANGELOG.md-016)

## 3. Journal schema (what 47 §4 is about) change history (sem: SEM-CHANGELOG.md-017)

**That `gx replay`'s deterministic replay agrees between the old and new binary** is the precondition for an upgrade. ∴ only the windows where `EngineJournalRecord`'s shape changed are listed here. Where a reader learns "which shape a directory was written with" is `.gx/VERSION` (`req/56` §2; `crates/gx-cli/src/layout.rs`'s `LAYOUT_VERSION`). (sem: SEM-CHANGELOG.md-018)

| Date | Window | What changed | Compatibility | Primary source (sem: SEM-CHANGELOG.md-019) |
|---|---|---|---|---|
| 2026-08-10 | **M6 hand 1** (included in tag `req0.08`; commit `f763a5b`) | **E-M5-13**: added `locator: String` and `parents: Vec<TransformationId>` to the `Planned` record | 🔴 **Breaking**. A journal file written before this commit does not read | `req/88` **M6-14** adopted (a); `req/38` §47; the `Planned` variant of `crates/gx-engine/src/store.rs` (sem: SEM-CHANGELOG.md-020) |
| 2026-08-15 | **v0.3-a A-2'** (two-phase escrow; `req/162`) | vocabulary 13→15 (new `ApplyObserved`/`InverseCompleted`) + `InverseEscrowed` gains `pending: bool` (serde default; `false` is omitted on the wire) | **Backward compatible**: an old journal reads on the new binary (`pending` defaults), and the old record's canonical bytes are unchanged too (`skip_serializing_if`) = 47 §4's replay agreement holds by construction. 🔴 Incompatible in the reverse direction only: a journal that carries a new record (written only by the declared pair's commit) does not read on the old binary | `req/38` §98 ruling 1; §99 ruling 2-①; `crates/gx-engine/src/store.rs`; `req/162` (sem: SEM-CHANGELOG.md-021) |
| 2026-08-16 | **R5** (`req/38` §165 ruling 2; DR-43-9; from the fifth adversarial audit `req/227` H-01) | **The record's shape did not change. The file's framing did.** A journal now begins with an eight-byte format marker (`GXJRNL01`) and stores a 32-byte chain link after every record's payload: `link_i = BLAKE3(0x00 ‖ "gx.journal.chain.v1" ‖ link_{i-1} ‖ payload_i)`. No variant was added, removed or reshaped — `EngineJournalRecord` is byte-identical on the wire and every CID over it is unchanged | **Both directions read**. A journal written before this window has no marker, is detected by its absence, and is read **and appended to** in the old framing (gx does not rewrite an append-only file); what it does not get is the chain, and 43 §7.7 says what its records are worth (only the ones the ledger backs). 🔴 The reverse does not hold: a marked journal does **not** read on a binary that predates this window — its first four bytes exceed the record ceiling, so an old reader stops at byte zero rather than mis-reading a record | `req/38` §165 ruling 2; 42 §3.13's v0.4-r note; `crates/gx-engine/src/replay.rs` (`JOURNAL_MAGIC`, `link`); `req/227` H-01 |
| 2026-08-16 | **R6** (`req/38` §167 ruling 2; DR-43-11 + DR-43-10 minimal; from the sixth adversarial audit `req/229` H-01/H-02) | **Neither the record's shape nor the file's framing changed.** What changed is `.gx/`: `VERSION` gains `key=value` lines after its first line (this window uses one, `journal_format=chained\|legacy`), and `checkpoints/head.json` arrives — a signed `Checkpoint` (42 §3.11, unchanged) plus the journal's length and chain head, rewritten atomically after every commit. `EngineJournalRecord` is byte-identical on the wire, `GXJRNL01` and the 32-byte links are byte-identical, and every CID is unchanged | **The journal reads both ways exactly as R5 left it.** What is new is a project-directory direction: a binary that predates this window parses the whole of `.gx/VERSION` as one number and refuses a project carrying a second line, so 🔴 **a project written by this binary does not open on a binary older than this window** — the same direction R5 already declared for the journal itself, one file over. `head.json` is ignored by older binaries (they never look for it) and its absence is never an error here (it means "this project has recorded no head") | `req/38` §167 ruling 2; 42 §3.13's v0.4-s note; 43 §7.8; `crates/gx-log/src/head.rs`; `crates/gx-cli/src/layout.rs`; `req/229` H-01/H-02 |
| 2026-08-16 | **R7** (`req/38` §171 ruling 2; 43 §7.9's Model A / Model B split; from the seventh adversarial audit `req/232` H-01/H-02) | **Neither the record's shape, nor the file's framing, nor `.gx/VERSION` changed.** What changed is the head document: `checkpoints/head.json` gains four optional fields — `version_digest` (the digest of `.gx/VERSION`), `ledger_leaf_hash`, `witness_signature` (a DSSE signature over the local numbers the `Checkpoint`'s own signature never covered, whose payload is rebuilt from the document rather than stored), and `accepted_rollback` (written only by `gx repair --accept-rollback`). `EngineJournalRecord`, `GXJRNL01`, the 32-byte links and every CID are byte-identical | **Both ways.** All four fields are `serde(default)`: a head written by R6 reads on this binary (its local numbers are then reported as unverified rather than as checked), and a head written by this binary reads on an R6 binary, which ignores the fields it does not know. The direction R6 declared for `.gx/VERSION` is unchanged and is not made worse | `req/38` §171 ruling 2; 42 §3.13's v0.4-t note; 43 §7.9; `crates/gx-log/src/head.rs`; `crates/gx-engine/src/pipeline.rs`; `req/232` H-01/M-02 |

| 2026-08-20 | **R29** (`req/38` §238 ruling 1; from the twenty-eighth adversarial audit `req/361` H-01) | **No variant of `EngineJournalRecord` was added, removed or reshaped.** What changed is a **value** inside one: `Aborted.rollback` is `Option<Rollback>`, and `Rollback` gains a fourth word, `Diverged`, written when 43 T-10c's roll-back was accepted by the adapter and the object was then read back and found **not** at the fingerprint the transformation started from. `GXJRNL01`, the 32-byte links and every CID are unchanged | **Backward compatible**: every journal written before this window reads on this binary — the three old words are untouched and nothing was renamed. 🔴 **Incompatible in the reverse direction, and this is the row that says so**: a journal carrying an `Aborted` record whose `rollback` is `Diverged` does **not** read on a binary that predates this window — `serde` has no arm for the word and refuses the record rather than guessing. 🔴 **That direction is reasoned from the derive's behaviour and is _not_ measured**: no binary predating this window was built and pointed at such a journal in this lane. It is written here as the honest expectation an upgrader should plan against, and the measurement is owed. This is the first schema window where a distributable is in someone's hands, so unlike every entry above it the damage is not structurally zero; the reverse direction is a real cost and is written down rather than discovered. `probes/doubt/tests/journal_changelog_doubt.rs` would **not** have caught this on its own (it watches the variant **name set**, and its own doc declares a change *inside* a variant as "not measured") — the row is here because the doc's stated condition, "it stops being acceptable the day a distributable exists", is now met | `req/38` §238 ruling 1; `req/361` H-01; `crates/gx-engine/src/store.rs` (`Rollback`, `Rollback::ALL_KINDS`); `crates/gx-engine/src/pipeline.rs` (`Engine::rollback_landed`); `docs/LIMITS.md` v0.5-p |
| 2026-08-20 | **R30** (`req/38` §240 ruling 3; from the twenty-ninth adversarial audit `req/372` M-02) | 🔴 **The framing gained a version because the *vocabulary* did.** A journal this build creates begins with `GXJRNL02` instead of `GXJRNL01`, with its own genesis link (minted over the new marker, so frames cannot be carried between the two). No variant of `EngineJournalRecord` was added, removed or reshaped and no record's bytes changed. Three guards come with it: a record whose vocabulary is newer than its journal's framing is **refused rather than written** (`EngineJournalRecord::minimum_format`); a file whose marker this build does not know is refused **whole** -- not truncated, not quarantined, not appended to, folded into `journal_intact` beside R6's downgrade; and `announce_quarantine` gained a sentence that names a newer vocabulary instead of describing a crash | **Forward compatible, and the reverse direction is the row.** Every journal written before this window -- `legacy` and `GXJRNL01` alike -- reads and appends on this binary exactly as before: what is refused is a *transition*, not a format. 🔴 **The reverse direction is now MEASURED rather than reasoned, and R29's row above owed exactly this measurement.** The twenty-ninth audit built a post-R29 journal and ran a **pre-R29 binary** (`3c2cf32`) against it. The older binary did **not** refuse it: `opened=Ok`, `records=1` of 3, `torn_tail_bytes=270`, the live file cut from **415 bytes to 145**, the removed bytes quarantined to `journal.bin.torn.145-415`, and after one ordinary append the journal read back as **2 records, torn=0** -- a healthy-looking journal with two records of history missing. The sentence it printed named `gx repair`, whose own documentation says what it cannot do is put them back. 🔴 **This window does not repair that, and cannot: no binary released before it is reached by anything written here.** A pre-R30 `gx` meeting a `GXJRNL02` journal still mis-frames it. 🔴 **Measured, not expected: the same `3c2cf32` binary was built again and pointed at a journal this build wrote, and the two roads give two answers, so both are published.** DECLARED (`.gx/VERSION` names the format -- every project `gx init` makes): **refuses**, `file_bytes 415 -> 415`, nothing quarantined, append refused in words, because a marker it cannot read makes the file sniff as `legacy` against a `chained` declaration and R6's downgrade guard fires. UNDECLARED (an embedder calling `EngineJournal::open`): **still truncates, and truncates more than before** -- `records=0 torn_tail_bytes=415`, cut `415 -> 0`, next append accepted, ending at 100 bytes reporting `records=1 torn=0`, so **315 bytes** gone where the pre-R30 journal lost **138**. That second row is a regression on that road and is not rounded up into the first: the marker moves the misreading earlier rather than converting it into a refusal, and no byte sequence this build can write would make an already-released binary refuse without a declaration to compare against. What is bought is the next version window and the ones after it. 🔴 **The cost, stated rather than left to be discovered**: a project created before this release keeps its `chained` framing (a chain cannot be re-framed in place -- the genesis link is minted over the marker), so on such a project an outcome needing a v2 word cannot be journalled: the record is refused and the verb fails instead of recording an abort. The exposure is small for a measured reason -- R30's other half removed the roads on which `Diverged` was easiest to reach | `req/38` §240 ruling 3; `req/372` M-02; `crates/gx-engine/src/replay.rs` (`JOURNAL_MAGIC_V2`, `JournalFormat::ChainedV2`, `genesis_link_v2`, `framing_this_build_does_not_know`); `crates/gx-engine/src/store.rs` (`EngineJournalRecord::minimum_format`, `EngineJournal::from_a_newer_gx`); `docs/LIMITS.md` v0.5-q |
| 2026-08-22 | **S③ AC-6** (`req/38` §283 ruling 5; `req/493` §1 AC-6; `req/497` §7) | **No variant of `EngineJournalRecord` was added, removed or reshaped, and no framing changed.** What changed is a **value** inside one: `ProvenanceDerived.provenance.environment` gains `confinement: Option<ConfinementContext>` — the kernel-confined bit and the ruleset hash `gx confine` took. It is journalled rather than derived because `ReceiptPayload` gains the same seat and 43 §7-3b compares a rebuilt payload's digest against the leaf the ledger holds: the process that repairs is not the process that committed, so a value re-read from the environment at rebuild time would answer `payload_mismatch` — the word for tampering — for every crash recovery of a commit made inside a `gx confine`. `GXJRNL02`, the 32-byte links and the vocabulary are unchanged | **Backward compatible in the direction that matters**: the field is `#[serde(default)]`, so every journal written before this window reads on this binary and rebuilds to `None` — an absence, not a fabricated `false`. 🔴 **The reverse direction is reasoned and NOT measured**: a journal whose `ProvenanceDerived` carries the key is expected to read on a binary predating this window, because serde ignores unknown map keys by default and no `deny_unknown_fields` sits on `Environment` — but no older binary was built and pointed at one in this lane, and R29's row records what that owing cost last time. 🔴 The **receipt** wire moved in the same window and that is a separate fact with a separate compatibility: `ReceiptPayload` is a map of sixteen where it was fifteen, so a payload this build re-encodes digests differently (`crates/gx-witness/tests/receipt_verdict_wire.rs` pins the fifth state of the one fixture beside the previous four). Unlike DR-46-28 it does **not** add a document to `docs/LIMITS.md`'s "required with no default" limit. 🔴 **Which half carries that was measured**: the serde default was removed and the compatibility probe stayed green, so the `Option` is what keeps August 2026's receipts decodable and the attribute is the declaration made explicit (`req/519` §7-4 recorded the same from the other side). `crates/gx-witness/tests/confinement_attest.rs` holds the attribute by a source scan rather than by the decode probe, and says so | `req/38` §283 ruling 5; `req/493` §1 AC-6; `req/497` §7; `crates/gx-witness/src/provenance.rs` (`Environment::confinement`); `crates/gx-witness/src/receipt.rs` (`ConfinementContext`); `crates/gx-engine/src/pipeline.rs` (`Engine::with_confinement`, `derive_provenance`, both rebuild roads); `crates/gx-cli/src/confine.rs` (`CONFINEMENT_ENV`) |
| 2026-08-23 | **DR-46-33** (`req/38` §413; `req/590`) | **No variant of `EngineJournalRecord` was added, removed or reshaped, and no framing changed.** What changed is a field inside one: `Planned` gains `input_generation: BoundaryStage` — the input-generation stage a deployment declares (its catalogue's `$determinism_boundary` slot, carried into the engine by the **optional** `InputStageDeclaration` trait so N-08's seven `SubstrateAdapter` methods are untouched), joined at plan time with the transformation's `Actor` (an `Actor::Agent` is an LLM origin whatever a static file declared) and journalled as the join's **result**. It is journalled rather than re-derived at rebuild time because 43 §7-3b compares a rebuilt payload's digest against the leaf the ledger holds, and neither the actor (Σ does not carry it) nor the catalogue (`gx-engine` does not depend on it) is reachable from a rebuild — only the result is. `GXJRNL02`, the 32-byte links and the vocabulary are unchanged, and **Σ does not move** (no `StateRow` field, no `reconstruct` arm reads it, so AC-039's bit-equality is untouched) | **Backward compatible in the direction that matters**: the field is `#[serde(default, skip_serializing_if)]` with `BoundaryStage::Unknown` its default, so every journal written before this window decodes as `Unknown` — v0's `attested_boundary` value, so a rebuild over an old journal is byte-identical — and re-encodes to the same bytes (`journal_roundtrip.rs` / `r30_journal_backward_compat.rs` hold). 🔴 **The reverse direction is reasoned and NOT measured**: a journal whose `Planned` carries the key is expected to read on a binary predating this window (serde ignores unknown map keys by default and no `deny_unknown_fields` sits on the record), but no older binary was built and pointed at one in this lane, and R29's row records what that owing cost last time. The **receipt** wire shape does not move — `determinism_boundary` (DR-46-28) was already a `ReceiptPayload` seat; what changed is the value of its `input_generation` stage, so a commit under a registered declaration or an agent actor now digests differently, pinned by `crates/gx-engine/tests/dr4633_input_generation.rs` (the flipped-value bed is refused, the kept-value bed is filed) | `req/38` §413; `req/590`; `crates/gx-engine/src/store.rs` (`Planned.input_generation`); `crates/gx-substrate/src/declaration.rs` (`InputStageDeclaration`); `crates/gx-engine/src/pipeline.rs` (`Engine::register_input_stage_declaration`, `joined_input_generation`, `journalled_input_generation`, `attested_boundary`); `crates/gx-adapter-mcp/src/adapter.rs`; `req/spec/40-architecture/42-data-model.md` §3.13 |

🔴 **Why M6 hand 1 was the window (a window, not a cost)** -- `req/88` M6-14, verbatim: "**M6 is the hand that makes the first distributable** -- break it before distribution and the cost is 0; break it after and every user's journal is the target". **The distributable was not yet in anyone's hands, so the real damage of this breaking change is 0.** That is luck, not discipline, so it is recorded here. (sem: SEM-CHANGELOG.md-022)

**A change not in this table means "it did not happen"** -- the other errata that touched `EngineJournalRecord` (E-M5-1's new `ApplyStarted`, M5-25 adopted (a)'s new `ProvenanceDerived`, E-M5-9's move to `Option`, M5H4-2's `rollback`, M5H6-2's `reason`/`actor`) **all happened inside M5**, in a period when not one user held a journal. ∴ there is nothing that should be written as "broke". **Writing that fact down is itself necessary, or the next reader cannot tell "a missing record" from "nothing happened".** (sem: SEM-CHANGELOG.md-023)

## 4. Milestone tag history (mechanically transcribed from `git tag -l`) (sem: SEM-CHANGELOG.md-024)

| tag | Date | Kind | subject (transcribed) (sem: SEM-CHANGELOG.md-025) |
|---|---|---|---|
| `req0.00` | 2026-08-06 | annotated | baseline: spec 25 doc + probes 32 green + lake build RC=0 (2026-08-06) |
| `req0.01` | 2026-08-06 | annotated | scaffold: 36 probes green (typed parser), e2e floor, semantics map — hostile-audited (B1-B5 fixed, verify PASS) |
| `req0.02` | 2026-08-07 | lightweight | req(08): V§10 NEW-B1/M1検収PASS——req0.02成立裁定(spot e2e RC=0 GREEN 37/5・判別pass確認) |
| `req0.03` | 2026-08-07 | lightweight | req(48): req0.03 pipeline完遂——手6b両半分GREEN(153/33判別つき+lake)・手6c RUNTIME実測(encode522/decode394/cid601ns median N=1000)・tag発行裁定 |
| `req0.04` | 2026-08-08 | annotated | M2 complete: gx-log merkle ledger + gx-witness DSSE receipts. Floor 370/62, AC 12/12, mutation 90.9%, Kani 3/3, fuzz 3 targets crash 0, primaries byte-checked. T4/T5 not yet proven (M8). |
| `req0.05` | 2026-08-09 | lightweight | req(38): §27 M3 fix批検収PASS+J-1..7裁定+tag req0.05成立宣言(偽PASS 6本全捕捉・coverage 5/5・床511/83) |
| `req0.06` | 2026-08-09 | lightweight | req(38): §36 M4 fix批検収PASS+L-1〜L-7裁定+tag req0.06成立宣言(coverage 8/8 PASS・battery 11本独立再走・規律46/47・gotcha45) |
| `req0.07` | 2026-08-10 | annotated | M5 complete: gx-engine state machine (21/21 transitions), all 22 ACs, floor 968/170, E-M5-1..16, crash recovery proven, decay identified, mutants+coverage measured. req/38 §37-§46. |
| `req0.08` | 2026-08-11 | lightweight | req(38): §56 M6 fix批検収PASS(凍結計器独立再走: 床1211/221一致・battery13/13 RED・ci GREEN/clone e2e/semantics5/5)+M6FIX-1追認/2はM7 FR化/6 shard禁+規律54(UC三分法)制定+Λ7 routing(req/97§4正本+axiom independence註記)+M6FIX-7起票+tag req0.08 |
| `req0.09` | 2026-08-13 | lightweight | req(38): §65 fix批検収PASS+B-1〜B-11裁定+req/103引用訂正+M7完遂=tag req0.09 |

🔴 **On `req0.00`'s subject spelling "spec 25 doc"**: that was the real count at the time; the canon later became 28 docs (`req/semantics.json`'s `spec.canon` title was corrected to the measured figure in v0.2.4 §68 #6). **This is a transcription, not a current claim**, so the tag's wording is not fixed -- a tag is the record of the moment it was struck. (sem: SEM-CHANGELOG.md-026)

🔴 **`req0.02`/`req0.03`/`req0.05`/`req0.06`/`req0.08`/`req0.09` are lightweight tags (an alias to a commit), and `req0.00`/`req0.01`/`req0.04`/`req0.07` are annotated tags (they carry a tag object)**. The mix was not intentional, it is just how things happened, and **the "Date" column above reads the tag object's date for an annotated tag, and the date of the commit it points at for a lightweight one** (`%(creatordate:short)` returns both through the same column). If numbers are going to be laid out, what date they are has to be written down. (sem: SEM-CHANGELOG.md-027)

## 5. Rules for the hand that next adds to this file (sem: SEM-CHANGELOG.md-028)

1. **A journal-schema change gets 1 row in §3.** Record the commit hash and that a journal written before that shape no longer reads (47 §4). (sem: SEM-CHANGELOG.md-029)
2. **A breaking change that bumps `0.y.z`'s minor gets 1 row in §2 or the new version's section** (NFR-024). Write "what broke". Not "improved". (sem: SEM-CHANGELOG.md-030)
3. **Do not write something as published when it has not been.** §1's disclaimer stays until that state changes. (sem: SEM-CHANGELOG.md-031)
4. **Take numbers from the machine** (`git tag -l`, `git log`, the real manifest). The summary text is a transcription, not an evaluation. (sem: SEM-CHANGELOG.md-032)
