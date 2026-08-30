# /limits — what gx does not cover yet

This page comes before the pitch, on purpose (`req/spec/20-business/21-business-plan.md`
§10-0's first rule: say what you do not guarantee before you say what you do).
`gx limits` prints the same eight lines at a terminal. The prose below is **verbatim** —
copied character for character from `crates/gx-cli/src/limits.rs`'s `LIMITS` constant,
not paraphrased for the page — so that `crates/gx-cli/tests/limits_sync.rs` can check the
two never drift apart with a plain substring match rather than a judgement call about
whether two different sentences "mean the same thing."

Nothing on this page is new. Each line paraphrases a clause already written in
`req/spec/40-architecture/45-threat-model.md` (the threat model, which is the one place a
limit is *created*) or in a closed implementation report; the citation after each line names
the source rather than restating this page's own authority.

1. if a policy does not say what you actually meant, gx enforces the policy exactly as
   written -- it checks that a change satisfies the rule, not that the rule was the right
   one to write (the Oracle problem).
   (45 §2.1 TH-1 / §4.1)

2. an attacker with root or kernel privilege can write around gx entirely, and this build
   does not detect it.
   (45 §2.2 TH-2)

3. revoking a key does not undo what was signed while it was still trusted -- how far back
   a revocation reaches is a setting an operator chooses, and gx only checks that
   everything after that point is consistent with it.
   (45 §3.1 candidate 13 / TH-5)

4. gx cannot see where an MCP tool's effect actually lands once a call reaches the server
   -- an agent that starts its own server, or a call that only reads (`resources/read`),
   is outside this build's proxy by declared design, not by an omission nobody noticed.
   (45 §3.1 candidate 14; req/119 §2.6 roads B-2 and B-9)

5. gx normalises a resource address the way RFC 3986 does, but it cannot see a server
   that quietly maps one path onto another underneath it -- if a server resolves
   `file:///srv/x` to `/etc/passwd` internally, gx never learns.
   (45 §3.1 candidate 12; req/119 §2.6 road B-10)

6. a verdict count only proves nothing was hidden from it -- it does not prove the policy
   behind it was strict. weaken the policy to admit everything and the count still reads
   clean, and two verifiers can legitimately be shown two different chains.
   (32 FR-M04 limits ①②)

7. `gx undo` only works where the server it asks exposes a real inverse for that tool --
   measured across a first census of public MCP tools, that held for about 13.8% of the
   ones that write anything at all; the rest have no restoring call to send, and gx
   cannot invent one.
   (req/38 §78 (census v2 stage1, first measurement))

8. the Lean model under `lean/` proves 117 theorems about the F0 specification, 12 of
   them named counterexamples, over 13 files, with 0 `sorry` and 1 `axiom` carried
   rather than proved (a line-start count of the tree itself, which a test re-takes on
   every run); the Rust implementation is
   compared against that model by a differential test -- 1,500 conformance vectors,
   six kinds, on every push -- and a comparison is a difference check, not a proof: no
   refinement theorem connects the two. as of 2026-08-29 (`req/908`), CI has run zero
   jobs on any push since 2026-08-15T17:25:29Z -- 13 days and 2,245 commits with no
   machine signal, because the GitHub Actions account is billing-blocked, not because of
   a code defect; the last time it ran (commit `f65aac2f`), the checks covered 16 of
   this project's 17 workspace crates automatically (`probes/doubt` runs by hand because
   its subject lives outside the repository), and the TypeScript SDK's own tests have
   never run under CI at all -- their green has always been a person's run, not a
   machine's.
   (45 §4.2 (v0.2.3 note; v0.4-l present-tense note); 51 §11.1 (v0.2.3 note; v0.4-l present-tense note); req/908 (2026-08-29 CI diagnosis); req/38 ERRATA SS871)

   > 🔴 **The two numbers in the first clause are stale, and this note is the correction**
   > (v0.5-f, `req/279` L-01). `README.md` says **92 theorems**, this line says **eight**, and a
   > buyer reads both. A primary recount at `da89e8d`, over all twelve files under `lean/`:
   > `grep -rhE '^(theorem|lemma)' lean --include='*.lean' | wc -l` = **91** (all `theorem`, no
   > `lemma`); `grep -rhE '^theorem [^ ]*counterexample' lean --include='*.lean' | wc -l` = **10**;
   > `grep -rn 'sorry' lean --include='*.lean' | wc -l` = **0**; one `axiom` at line start
   > (`GxSpec.composeId`). The README's 92 counts one line of English prose inside a block comment
   > in `MinimalityF0.lean` that begins with the word `theorem`, which a leading-whitespace-tolerant
   > pattern picks up; **91 / 10 / 0** is the honest count. The sentence above is left **verbatim**
   > because `gx limits` prints it from `crates/gx-cli/src/limits.rs` and a machine gate
   > (`crates/gx-cli/tests/limits_sync.rs`) holds the terminal and this page to the same words: the
   > number moves in that constant, in a lane whose write scope includes `crates/gx-cli`, and this
   > note stands until it does. Whether "eight" ever meant a named subset about the F0 specification
   > rather than the whole model is not decided here; what is decided is that the page says which
   > count is primary.
   >
   > **Resolved in this window** (2026-08-18, `req/38` §207 ruling 2, `req/288` L-01b): the note
   > above is left standing, word for word, because it is the record of a page that was wrong about
   > itself for a release -- but the sentence it points at has moved. Item 8 and `README.md` now
   > carry **91 / 10 / 0 / 1 over 12 files**, and `crates/gx-cli/tests/limits_sync.rs` re-takes that
   > line-start count from `lean/` on every run and refuses any of the three faces that has drifted
   > from it. What replaced the stale pair is not a fresher number; it is a number nobody has to
   > remember to update. Two faces this lane could not reach are named in `req/289` §4:
   > `public/README.md` and `public/org_profile/README.md` still print **92 / two axioms** and
   > **90 / three axioms**, and both are outside its write scope.

   > 🔴 **B8 correction (2026-08-29, `req/908`, `req/910` B8, `req/38` ERRATA SS871): the tail
   > clause's present tense went false on 2026-08-15T17:25:29Z and stayed false for 13 days
   > before anyone noticed.** The struck sentence below is the wording this line carried from
   > `req/288` L-01b (2026-08-18) until this correction; it is kept, not deleted, because a
   > limits page that silently rewrites its own history is asking to be trusted about the
   > present the same way it was just caught not being:
   > ~~of the checks that run automatically on every push today, 16 of this project's 17
   > workspace crates are covered (`probes/doubt` runs by hand because its subject lives
   > outside the repository) and the TypeScript SDK's own tests are not run by CI at all --
   > their green is a person's run, not a machine's.~~
   > **What actually happened**: the last green CI run was `31898083900` (`ci`, 2026-08-15T17:21:42Z,
   > commit `f65aac2f`); the first red run, `31898263122`, followed **3 minutes 47 seconds later**
   > (commit `44571dfe`) and every run since -- all three workflows (`ci`/`nfr-ci`/`lean-nightly`),
   > 13 days, 2,245 commits -- carries the identical scheduler annotation: *"The job was not started
   > because recent account payments have failed or your spending limit needs to be increased."*
   > Job "duration" is 2-5 seconds on every failing run, which is the scheduler refusing the job
   > before assignment, not execution time -- there is no log for any of them because none ever ran.
   > `req/908` verified this directly (`gh run view` on the run objects, not inferred from logs) and
   > corroborated it locally: `cargo check --workspace --all-targets` (18 crates + `probes/doubt`)
   > passes clean at HEAD, but `fmt`/`clippy`/`nextest` -- what `tools/ci.sh` actually gates -- were
   > not run in that check, so **compiling clean is not the same claim CI green would have been**.
   > This is a **billing/spending-limit lockout on the GitHub account, not a code defect** -- the
   > fix is an operator opening Billing & plans, out of any lane's write scope. What is true right
   > now, and what the live sentence above says: CI ran the design this struck sentence describes
   > the last time it ran at all (2026-08-15, `f65aac2f`), and has produced zero signal about any of
   > the 2,245 commits since. **`release.yml` -- the only path that cuts a version -- has never run
   > at all** (`gh run list -w release.yml` returns `[]`), separately from and not part of the red
   > streak. A local substitute for the machine-checkable parts of CI (everything except the ARM/
   > macOS runners) was run separately and is not what this note is about; it does not change what
   > the live sentence above says about the public, continuously-running gate itself.

   > v0.4-l (2026-08-15, `req/189` H-13 -- three faces, req/38 §120): **theory** = eight Lean
   > theorems + five counterexamples, kernel-checked; **mechanised, not proven** = the Rust <->
   > Lean differential test (1,500 vectors, six kinds, every push -- a comparison finds
   > differences, it does not establish equivalence; no refinement theorem exists);
   > **implementation** = ci.yml runs `tools/ci.sh` over 16 of 17 workspace crates on every push
   > (`probes/doubt` by hand), the TypeScript SDK's tests outside CI. The previous eighth line is
   > kept below, struck, because a limits page that silently rewrote its own history would be
   > asking to be trusted about the present:
   >
   > ~~8. checking this Rust implementation against an independent Lean proof is planned for a
   > later milestone and does not exist yet; and of the checks that run automatically on every
   > push today, only 2 of this project's crates are covered by them -- the rest are green because
   > a person ran the checks, not because a machine enforces it on every change.
   > (45 §4.2 (v0.2.3 note); 51 §11.1 (v0.2.3 note))~~ -- stale since M8 (lean/) and v0.2.7 batch
   > (ci.yml scope widened, req/137 §B1 item 8); measured `req/182` H-13.

   > v0.4-n (2026-08-16, `req/38` §150, R1 `req/213`): **landed since the note above -- `gx
   > serve` and the `gx` CLI can now hold one project's `.gx/` open at the same time.** A second
   > writer no longer corrupts the ledger the way `req/182` H-01 first measured (a leaf silently
   > dropped, `200` returned for a commit whose own inclusion proof then answered
   > `found:false`); it is turned away instead, with `503 BUSY` (`Retry-After: 1`, a
   > per-operation `.gx/LOCK`). Restarting the server no longer breaks list reads either: `GET`
   > after a restart returns `200` with a non-null row, checked end to end against a real binary
   > and socket by `serve_runtime_e2e`. **What restart recovery still does not give back**: the
   > row's `transformation` and `actor` stay `null` until the draft archive (R2) lands -- the row
   > exists and its id and creation time are real, its body is not, yet. `gx undo`'s behaviour
   > after a restart or after a third-party change is unchanged by this and is tracked
   > separately, unresolved (DR-43-1).

   > v0.4-o (2026-08-16, `req/38` §132 ruling 2 / §144 ruling 2, `req/216`): **the last sentence
   > above is resolved for the third-party half, and the resolution is a refusal rather than a
   > feature.** `gx undo` now compares the world against the observation the original commit
   > itself signed (42 §3.10's `postcondition_fingerprint`) and **refuses** when they differ:
   > exit 3 / `409 PRECONDITION_CHANGED`, nothing applied, nothing recorded. What that costs you,
   > said plainly: an undo is now blocked whenever *anything else* changed the target since the
   > commit -- including changes that were perfectly legitimate. gx does not decide who was
   > right; it declines to overwrite a change it cannot account for, and leaves the decision to
   > you. Before this, the escrowed inverse was written over the top and the other change was
   > gone with no message (measured: `req/182` H-15, on a file and on a git branch).
   >
   > Two honest edges. **Where the world cannot be observed** -- an MCP server that exposes tools
   > and no readable resources, or a commit whose receipt was not kept -- there is nothing to
   > compare, so the undo proceeds exactly as it did before and says on stderr which absence it
   > is. That is a declared gap, not a check. **Granularity is the adapter's**: the comparison is
   > the adapter's own content digest, so a change the adapter's digest does not cover (a file's
   > mode, its symlink target, its extended attributes) is not detected, and a change to anything
   > the adapter's scope *does* cover blocks the undo even when it is unrelated to the target.
   > The restart half of the sentence above is still open (R2's draft archive).
   > (`crates/gx-cli/tests/undo_cas_e2e.rs`, `crates/gx-adapter-mcp/tests/undo_cas_mcp.rs`)

   > v0.4-n (2026-08-16, `req/38` §146, `req/207` §1-4/§1-5): **third-party verification of a
   > receipt is now a machine-checked claim, not a reading of the code.** `gx receipt verify
   > --offline --checkpoint <FILE> --key <FILE>` runs correctly with no `.gx/` workspace, no
   > populated `HOME`, and no network -- one binary (`gx` itself) plus three files (receipt,
   > checkpoint, public key) is what a third party needs. This is a claim of sufficiency, not of
   > smallness: `gx`'s own dependency closure is not small (286 crates). The key and the
   > checkpoint stay two separate arguments on purpose -- folding a public key into the receipt
   > itself, the way one comparable external verifier does, would move the root of trust inside
   > the thing being verified, and a receipt with its signature stripped would pass -- neither is
   > true of this build (`crates/gx-cli/tests/receipt_verify_hermetic.rs`, three tests, checked
   > with no `HOME`, no workspace, and no network).

   > v0.4-o (2026-08-16, `req/222` §4 GO condition 6 / `req/182` H-09): **a receipt older than the
   > head verifies.** Until this repair, `gx receipt verify <FILE>` against the project's own
   > ledger answered `inclusion:"refuted"` for every receipt but the newest — an inclusion proof
   > reaches the root of the tree it names, and the default anchor is the tree as it stands, so a
   > log that merely grew was producing the word for tampering. It now carries an RFC 6962 §2.1.2
   > consistency proof from the receipt's tree size to the anchor's (made by the local ledger on
   > the default path, or handed in with `--consistency <FILE>` by a third party), and the root at
   > the receipt's own size is computed from the receipt rather than supplied — so the chain has
   > no unchecked link and the answer is `verified` rather than a softer word. **A leaf that is
   > not in the tree is still `refuted`, and no anchor is still not a pass.** What is new is a
   > fifth word, `unbridged`: the anchor and the receipt name different tree sizes and nothing
   > tied them together. It is not a pass (exit stays 7) and it is not evidence against the
   > receipt — the honest state between the two (`crates/gx-cli/tests/receipt_verify_history.rs`,
   > six tests).

   > v0.4-p (2026-08-16, `req/38` §160 ruling 2, from the third adversarial audit `req/222`):
   > **an undo whose precondition cannot be checked is now refused rather than fired**, and three
   > other things a running project used to get wrong are named here rather than left to be
   > found. (1) The compare-and-set above reads the commit receipt under `.gx/receipts/`. Until
   > this release nothing verified that document: deleting one file disabled the check silently
   > and the undo overwrote whatever was there, and a receipt copied from another transformation
   > was accepted as this one's evidence. The receipt is now checked four ways -- present,
   > decodable, DSSE-signed by this project's key, and about this transformation -- and any
   > failure is `409 PRECONDITION_CHANGED` / exit 3. **A deployment that keeps no receipt archive
   > therefore cannot undo at all**, which is the honest form of what was always true. The `200`
   > answer now carries `witness: attested | unobservable:<reason>` so that a caller can tell a
   > checked undo from an unchecked one. (2) **A commit whose receipt the archive will not take
   > is reported as a failure** (`500`), naming both halves: the change was applied and the
   > receipt was not filed, so that row cannot be undone until it is. (3) **Deadlines survive a
   > restart** -- 43 T-6's TTL used to be evaluated only against rows the running process had
   > planned itself, so restarting a server silently cancelled every deadline in the project.
   > (4) **A ledger rewritten without changing its length is detected**: the change detector now
   > compares the last record's bytes as well as the file's length, and a writer re-reads the
   > ledger from disk under the project lock before writing. What is still not detected is a
   > rewrite in the *middle* of the file while only reads are happening; the next write catches
   > it. A project whose journal and ledger disagree now has a verb -- `gx repair` reports what is
   > wrong and `gx repair --yes` runs the recovery under the lock -- and that verb says plainly
   > what it cannot do: **a leaf the ledger lost cannot be rebuilt from the journal**, because the
   > journal records no receipt digest, and a leaf invented here would be a signed lie about a
   > Merkle tree. (`crates/gx-cli/tests/serve_runtime_r3.rs`, seven tests, all of them red on the
   > commit before this one)

   > v0.4-q (2026-08-16, `req/38` §163 ruling 2, from the fourth adversarial audit `req/225`):
   > **the three repairs above each had a hole, and the audit found all three in the code the
   > previous release added.** They are named here in the order an operator meets them. (1)
   > `gx repair` without `--yes` said it wrote nothing and **wrote**: it opened the project
   > through the door that quarantines a tail that will not replay and then cuts it, which on the
   > one class of project this verb is for took a 522-byte ledger to zero. Run beside a live
   > server, that took `/healthz` from `200` to `500`. Without `--yes` the project is now opened
   > read-only and not one record moves; the one thing the report still writes is the lock file's
   > own "who is here" note. The release note that said a running `gx serve` makes this verb
   > answer `BUSY` was also wrong and is corrected: the project lock is per-operation, so an idle
   > server holds nothing, and what makes the report safe beside one is that it writes nothing.
   > (2) **On a deployment whose engine key is not its actor key — which is what `gx serve
   > --signing-key` is — every `gx undo` was refused**, permanently, with a message accusing an
   > authentic receipt of not verifying. The receipt is now checked against the key it names
   > (the signed payload carries it) rather than against the key the caller happens to hold, so
   > a key rotation also stops killing the undo of every commit made before it. A receipt naming
   > a key this machine does not hold is refused **by that name** rather than reported as a bad
   > signature. (3) **The same-length-rewrite detector was on the ledger only.** One bit flipped
   > in a live project's journal and the server went on answering `200`, accepting writes and
   > signing checkpoints, until the next start-up refused to open the project at all. The journal
   > now carries the same two checks: its last record's bytes on every read, and a replay of
   > everything already read on every write. A journal that has become **shorter** than what was
   > read is treated the same way, which closes the older complaint that `/healthz` reported `ok`
   > over a truncated log. What is still not caught by a read alone is a rewrite in the *middle*
   > of either file; the next write catches it, and that limit is asserted rather than hoped for.
   > (`crates/gx-cli/tests/serve_runtime_r4.rs`, twelve tests, ten of them red on the commit
   > before this one)

   > v0.4-r (2026-08-16, `req/38` §165 ruling 2, from the fifth adversarial audit `req/227`):
   > **the detector the last release gave the journal compared a shape, and a shape is not an
   > identity.** It asked whether the same number of bytes came back as the same number of whole
   > records. Three rewrites satisfy both counts, and the audit ran all three against a live
   > server: a record overwritten with the bytes of **another record from the same file** (one
   > commit writes ten records of the same framed lengths every time, so from the second commit
   > onward every record has a twin — the tool is `cp`, no key and no encoder needed), two
   > adjacent records **swapped**, and one bit flipped inside a payload. In all three the server
   > answered `200` on `/healthz`, `201` on new candidates and signed checkpoints over the file,
   > `gx repair` called the project healthy, and `gx replay` answered `matches: true`. **And the
   > damage did not stop at the bookkeeping**: where the substituted record was the one that
   > records a completed commit, the next start-up read that commit as unfinished, asked the
   > adapter to apply its change **again**, and the operator's file went from `three` back to
   > `one` — with every health check green on both sides of it.
   >
   > Two repairs, deliberately of different kinds. **(1)** Every record in a journal now carries a
   > 32-byte link over its own bytes and its predecessor's link, and the file begins with an
   > eight-byte format marker. "Is this the file I read" became one comparison of 32 bytes, no
   > rewrite of any size survives it, and — because verifying links decodes nothing — it is now
   > cheap enough to run on **reads** as well as writes (measured, debug build: 7.6 ms over
   > 10,105 records / 1.1 MB, against 35.2 ms for the old shape check over the same content). So
   > `/healthz` stops saying `ok`, which is the whole point of a health check. A journal whose
   > chain is broken is **not** truncated: everything after a break is a whole record, and gx
   > cannot put back what it cuts. **(2)** The recovery that runs at start-up — the one road on
   > which gx writes to your files without being asked — now refuses twice: it does nothing at
   > all on a journal it cannot trust, and it will not re-apply a commit the ledger no longer
   > puts last, because a commit with later commits behind it is not one that was interrupted.
   >
   > **What this does not fix, stated plainly.** A journal written by an earlier version has no
   > links, and gx does not rewrite it — an append-only file that gets rewritten is not one. Such
   > a journal is read and appended to in its own format, `gx repair` reports it as `legacy`, and
   > the rewrite above is **invisible** there; ~~what still holds is that the recovery will not
   > write an old change back to your files.~~ 🔴 **That sentence was written without a condition
   > and it is false without one — see v0.4-s below, which measures the case and states the
   > condition.** And a journal whose links are recomputed end to end
   > by somebody who understands the format verifies perfectly, because a chain is a file's
   > argument about itself: what catches that is the ledger beside it, and this release's own
   > adversarial probe is the one that says so.
   > (`crates/gx-cli/tests/serve_runtime_r5.rs`, sixteen tests, the four that reproduce the audit
   > red on the commit before this one)

   > v0.4-s (2026-08-16, `req/38` §167 ruling 2, from the sixth adversarial audit `req/229`):
   > **the last release gave the journal an identity. An identity is not a promise that the
   > history has not been shortened.** A chain link commits to its record and every record before
   > it, so the first *n* records of a chained journal are themselves a perfectly chained journal
   > — and the ledger beside it is a sequence of framed leaves, so its beginnings are perfect
   > ledgers too. The audit needed no key, no encoder and no forgery: it cut both files at a
   > record boundary with `truncate`. The result passed everything. `gx repair` answered exit 0
   > with "the journal is intact" and no remedy, the server started, `/healthz` said `ok`, and
   > `GET /ledger/checkpoint` returned a **signed** head over the shortened tree. Where the cut
   > fell between a ledger entry and the record that closes it, the start-up recovery read the
   > surviving commit as unfinished, applied its change **again**, and the operator's file went
   > from `three` back to `two` — with the start-up line's "refused: 0" beside it. A second
   > finding: the chain can be taken off from the outside. Strip the marker and the links, and
   > the whole apparatus of the last release stops existing — `gx repair` calls the file "legacy"
   > and healthy, the server starts silently, and the rewrite the chain was built to catch works
   > again.
   >
   > **Three repairs.** **(1)** Every write now records where this project has reached — a signed
   > checkpoint plus the journal's length and chain head — in `.gx/checkpoints/head.json`. Opening,
   > catching up and `gx repair` compare the project in front of them with that record and refuse
   > one that is shorter or that is not an extension of it. The recovery does not run at all on a
   > project in that state. **(2)** A project now records which journal format it is in, so a
   > chained project that suddenly has no chain is refused instead of accepted as an old one.
   > Projects written by earlier versions are unaffected: what is refused is the *change*, not the
   > format. **(3)** `gx checkpoint export <FILE>` copies the signed head out of the project, with
   > no key, and `gx repair --against <FILE>` reads it back.
   >
   > **What this does not fix, stated plainly, and it is the important paragraph.** The first two
   > repairs live **inside** the project directory, which is inside the reach of anyone who can
   > write to the two files they protect. Delete `.gx/checkpoints/head.json` and the rollback is
   > invisible from inside the project again — gx will say, correctly and uselessly, that this
   > project has recorded no head. Delete the format line from `.gx/VERSION` and a downgrade is
   > indistinguishable from an old project. Both are measured, and the tests that measure them
   > assert that gx **passes** the project (`serve_runtime_r6.rs`, `s1_`/`s2_`). So the honest
   > statement of what gx offers here is conditional, and the condition is a habit rather than a
   > feature: **keep a copy of the signed checkpoint, and of your commit receipts, somewhere the
   > project cannot reach.** With that copy, a removed commit is not merely suspected — it is
   > provable in two commands: the receipt verifies against the checkpoint you kept and is
   > `refuted` against the project's own ledger. Without it, an attacker who can write to your
   > `.gx/` can leave you a project that is internally perfect and shorter than it was.
   >
   > **What it costs.** The integrity check is **61 ms per megabyte of journal end to end** on a
   > write, not the 6.7 ms the last release's note quoted — that figure measured the chain walk
   > alone and not the road a writer takes. Recording the head adds a further **~14 ms per commit**,
   > flat: one signature, one small file written and fsynced, one rename, one directory fsync. Both
   > numbers are from an unoptimised build on ext4, and both are stated because a check nobody
   > priced is a check somebody will switch off.
   >
   > One smaller correction in the same release. `gx repair`'s
   > `repaired` field used to report the flag you passed rather than whether anything was written;
   > it now reports what happened, with the flag under `mode`.
   > (`crates/gx-cli/tests/serve_runtime_r6.rs`, fifteen tests, five of them attacks on this
   > release's own new code)

   > v0.4-u (2026-08-17, `req/38` §173, from the eighth adversarial audit `req/234`):
   > **the eighth audit measured only accidents — no attacker anywhere — and still found four
   > things wrong. Three of them needed nothing but a power cut, an editor, or a colleague.**
   >
   > The heaviest one was the receipt. A commit that had become durable in both of gx's files could
   > still lose its **signed receipt**: the archive was written after the commit returned, so a
   > power cut in that window left a committed row with no evidence anywhere, permanently.
   > `gx undo` then refused that row for ever, `GET /v1/receipts/{tid}` answered `404`,
   > `gx receipt verify` exited 6, **and `gx repair` called the project healthy**. The window was
   > about 44% of one commit. Two things changed. The receipt is now filed **inside** the commit's
   > critical section and **before** the record that makes the row final, so any crash lands in a
   > window the recovery already closes — and a commit whose receipt cannot be filed is now a
   > **failed commit** rather than a success with a missing document. And gx now does the
   > subtraction it always could: `gx repair` reports `commit_receipts`, `receipts_missing` and the
   > ids, with `gx repair --yes --reissue-receipts` to file what is missing. That last verb never
   > touches your files: it rebuilds each receipt from the world and refuses to sign unless the
   > result matches what the ledger already witnessed. And `gx repair` now **exits 1** when a
   > committed leaf has no receipt, because a status of `0` on a project that cannot show anybody
   > what it did is the half of the old answer that a monitor was reading. Measured: 39 mid-commit
   > power cuts across 13 offsets, zero receipts lost (it was three of seven offsets before); and
   > a deleted receipt, re-issued, takes a row that `gx undo` refused back to an undo that works.
   >
   > The second needed an editor and nothing else. `.gx/VERSION` is two lines of text, and its
   > digest was taken over the **bytes**, so one trailing newline, or a save that turned the line
   > endings into CRLF, or one trailing space, stopped a provably intact project from starting —
   > and the diagnosis said "its ledger, its journal, or both are shorter" and told you to restore
   > from a backup, on a project whose ledger and journal matched exactly. The digest is now taken
   > over what the file **declares**. Changing `journal_format=chained` to `legacy` is still
   > refused, which is what the check is for; whitespace is not. The remedy for that refusal now
   > names `.gx/VERSION`, says what the two correct lines are, and does not mention the ledger.
   > `gx repair` also prints `files_agree` beside `ledger_agrees_*`, because those are two
   > different questions and only the first one is about the two files.
   >
   > The third: `gx undo` waited for the world to settle **while holding the project's writer
   > lock**. If somebody else had touched the file, the undo polled for its whole budget — two
   > minutes by default — and for those two minutes every write to the project, the signing
   > endpoint, and `gx repair` itself answered `busy`, while the HTTP `/undo` answered the same
   > question instantly. The waiting now happens with the lock released; the reading before and
   > after it happens with the lock held. Measured: `gx repair` during a settle went from `busy` to
   > answering in 0.03 s. What this costs, stated: if another `gx` takes the lock while an undo is
   > waiting, that undo now ends in `busy` instead of holding everybody else up.
   >
   > The fourth is **not closed, and this is where it is written down.** Between the moment gx
   > checks that your file is what it expects and the moment it writes, there is a gap, and a write
   > that lands inside that gap is overwritten without gx saying so. This release makes gx check
   > again as late as it can: measured on this machine with `strace`, the gap went from **23.5 ms
   > to 14.3 ms** of a ~110 ms commit. It does not go to zero, and the reason is worth stating
   > plainly: **13 ms of the remaining 14.3 ms is two `fsync` calls** — one that records "gx is
   > about to write" and one that makes your file durable. Removing either would trade this gap for
   > a worse one. Closing it properly needs the check and the write to be a single operation, which
   > is an adapter-contract change and is filed as **DR-43-13** rather than done here. If your
   > workflow has another tool writing the same file while gx is committing it, that is the
   > 14.3 ms you are exposed to — on `fs`. On `git` the same gap exists and is **not measured**;
   > on `mcp` it cannot be measured at all, because gx cannot see the other side of the proxy.
   >
   > Two smaller ones. An escrowed inverse whose body somebody deleted from
   > `.gx/ledger/journal.blobs/` used to be reported as `Available` — gx saying a body was there
   > when it was not; it now reads the store before answering and says `BodyMissing`, and
   > `gx repair` counts them. There is **no repair** for that: an inverse is the only copy of what
   > a change replaced, and `gx checkpoint export` copies the head, not the bodies. And the CLI and
   > the HTTP API now give one sentence, not two, when a receipt cannot be filed.
   > (`crates/gx-cli/tests/model_a_probes.rs`, eight tests, three of them new and all three red on
   > the commit before this one)

   > v0.4-t (2026-08-16, `req/38` §171, from the seventh adversarial audit `req/232`):
   > **the last release added a detector and never checked it, and this release stops pretending
   > that one kind of attacker and another are the same problem.**
   >
   > Two findings. The first: nothing verified the signature on
   > `.gx/checkpoints/head.json`, so the record of where this project had reached did not have to be
   > *deleted* to be switched off — writing `{"tree_size": 0}` over it, leaving the signature where
   > it was, disabled every check while the report went on saying `head_recorded: true`. The pair
   > was then cut, the server started, `/healthz` said `ok`, and an operator's file went from
   > `three` back to `two`. The second finding needed no forgery at all: the head is rewritten at
   > every commit, so keeping **one copy** of a genuine head and putting it back after a rollback
   > gives a document that verifies under any check we can perform, and the head stops being a
   > fence and becomes a floor — every rollback that stops above it is invisible.
   >
   > **The split.** Those two are not the same problem, and answering them as if they were is what
   > produced five releases of one detector chasing another. So the threat model is now written in
   > two halves (43 §7.9), and each of them gets a different kind of answer:
   >
   > * **Accidents** — a crash, a power cut, a half-written file, a second process, a restart, a
   >   file edited by hand or by an older tool, a wrong backup restored, and anybody who cannot
   >   write to your `.gx/` directory. This is what the checks inside gx are **for**, and the goal
   >   here is zero. This release adds three: the head's signature is verified when a key for it can
   >   be found; the numbers beside it that the signature never covered (the journal's length and
   >   chain head, the digest of `.gx/VERSION`, the last leaf) now travel under a signature of their
   >   own, rebuilt from the document's own fields so that editing a number breaks the check by
   >   construction; and a project's declaration of its own format can no longer be **rewritten**
   >   (the last release only noticed it being deleted).
   > * **An attacker who can write to `.gx/`** — the same user account, or somebody who has got
   >   into it. **The checks inside gx cannot answer this and this release says so instead of
   >   implying otherwise.** They live in the directory they protect, so they can be deleted,
   >   replaced, or rolled back to an older genuine copy; and the last of those defeats signature
   >   checking outright, because a signature says *who* and never *when*. Against this, what
   >   answers is the copy that left the machine, and operating-system permissions — which gx does
   >   not yet use (filed as DR-43-12; today it is a design nobody has evaluated, not a feature that
   >   exists).
   >
   > **What that changes in what you read.** `head_recorded: true` now means "a head file is here",
   > not "the head checks out": the second question is `head_authenticity`, which answers `absent`,
   > `unverified` (this machine holds no key for the id that document names — **not** a pass),
   > `verified`, or `refuted`. `gx repair --against <FILE>` compares `origin` and `key_id` before it
   > compares trees, so another project's checkpoint is now named as foreign instead of being
   > believed. `gx checkpoint export` verifies the head before it copies it and refuses to write a
   > document that does not check out. `gx log checkpoint` will not sign a tree the recorded head
   > contradicts. And a server signs a checkpoint only under the project lock, so two servers on one
   > project cannot produce a signed statement about a tree neither of them has: the cost is that
   > this one endpoint answers `busy` while another `gx` is writing.
   >
   > **The one new way to lose history on purpose.** A project that has gone backwards is refused
   > everywhere, and until now the only way out was a backup. `gx repair --yes --accept-rollback
   > --against <FILE>` takes the shorter tree deliberately: it requires a checkpoint kept outside
   > the project, refuses if that document says the tree was longer, and records what it replaced in
   > the new head. The previous release did this silently on every `gx repair --yes` over a project
   > with no recorded head, which is how a rollback became a project's attested past.
   >
   > **What this does not fix, stated plainly, and it has moved.** The last release's paragraph said
   > "delete `.gx/checkpoints/head.json`, delete the format line". ~~Delete~~ is one of **three**:
   > delete, replace, or restore an older genuine copy. Replacing is closed here; deleting and
   > restoring are not, and cannot be — a project cannot prove from the inside that the record it
   > holds is the *current* one, because the record and the thing it records are in the same place.
   > The tests measure both and assert that gx **passes** the project
   > (`serve_runtime_r6.rs::s1_`, `serve_runtime_r7.rs::h02_`) — and `h02_` also asserts what that
   > costs: the operator's file moves. So the honest statement of what gx offers is unchanged in
   > shape and clearer in order: **keep a copy of the signed checkpoint, and of your commit
   > receipts, somewhere the project cannot reach — take one after every commit, or at least once a
   > day.** With that copy, a removed commit is provable in two commands. Without it, an attacker
   > who can write to your `.gx/` can leave you a project that is internally perfect and shorter
   > than it was, and this release moves that sentence to the front rather than leaving it as a
   > habit at the end.
   > (`crates/gx-cli/tests/serve_runtime_r7.rs`, seventeen tests, five of them attacks on this
   > release's own new code; `crates/gx-cli/tests/model_a_probes.rs`, five tests, which are the
   > accident half in one file so that the next audit has a named surface to attack.)

   > v0.4-v (2026-08-17, `req/38` §175, from the ninth adversarial audit `req/236`):
   > **the ninth audit re-ran the eighth's four repairs, found three of them genuinely closed, and
   > then found four more things wrong right next to them. Every one of the four needed nothing
   > but an accident.** Two sentences in v0.4-u above are corrected here rather than edited out.
   >
   > **Correction 1, and it is the important one.** v0.4-u told you that an escrowed inverse whose
   > body was gone "is now read before answering" and that "`gx repair` counts them". Both were
   > false when they were written. The read was a check that the *file name* existed, not that the
   > body did — so a body that was **half there** was still reported as `Available`. And the count
   > could never be anything but zero: it was taken from a table that a freshly started `gx` leaves
   > empty, so `gx repair`, which is always a fresh process, was counting an empty list.
   >
   > **What that combination did, measured end to end with no attacker and no hand edits.** A full
   > disk during a commit left **204,800 bytes of a 400,096-byte inverse at its own address**. gx
   > never rewrote it, because its first question was "is a file there" and the answer was yes.
   > The next commit — a completely successful one, `rc=0`, signed receipt, `gx receipt verify`
   > clean — escrows the same inverse and therefore **adopted the fragment as its own undo**. From
   > then on `gx repair` said healthy, the API said `Available`, and `gx undo` failed for ever.
   >
   > Fixed, in four places. The blob write is now a temporary file, an `fsync`, a `rename` and a
   > directory `fsync` — the same shape the receipts and the head already used — so a body at an
   > address is either whole or absent. A body that is already there is only reused if its **bytes
   > match**. A commit checks that the inverse it just escrowed reads back before it moves your
   > world, and fails closed if it does not. And `Available` now means the body was read, decoded
   > and matched its own name. `gx repair` reports both halves: `escrow_bodies_missing` (rows whose
   > body is unusable, which now really can be non-zero) and `damaged_bodies` (files in that
   > directory that do not read back as the name they are filed under, which is what an older
   > release may have left you).
   >
   > **Correction 2: a wrong signing key could destroy a project, permanently.** This one is new
   > here and was in no previous list. gx's crash recovery rebuilds the signed receipt of a
   > half-finished commit and checks it against the ledger. The rebuild includes **which key
   > signed it** — so running the recovery under a different key could never match, and the
   > mismatch wrote a *final* record, which is the one thing the recovery stops at. One run under
   > the wrong key removed the project's only way out for good: `gx serve` would not start, the
   > change was already on your disk, and running the recovery again with the **right** key did
   > nothing. Measured: the committing key, 7 runs, 0 broken; another key, 8 runs, **8 broken**;
   > `gx serve --signing-key <another key>`, 7 runs, **7 broken** — and `gx serve --signing-key` is
   > the ordinary way to run a server. It now reads the key out of the receipt the project already
   > holds, and where there is no receipt yet it **refuses without writing anything**, names the
   > key you need, and the correct key still finishes the job afterwards.
   >
   > **Correction 3: five ways an editor could stop the project, including the diagnostic.** A
   > byte-order mark, a leading blank line, old-Mac line endings, a "Save as UTF-16" from Notepad,
   > or the two lines of `.gx/VERSION` swapped: any of them stopped `gx repair`, `gx log proof`,
   > `gx replay` and `gx serve` alike, with "is not a readable layout version" and **no report, no
   > remedy and no way out** — on a project whose ledger, journal and receipts were untouched. All
   > five are read correctly now, because the file is decoded (byte-order marks and UTF-16
   > included) and split into "the version line" and "the settings" before anything parses it, and
   > the version is whichever line is not a setting, so their order does not matter. A file that
   > still will not read gets a code of its own, `DECLARATION_UNREADABLE`, which says what shape
   > the bytes are in and what the two correct lines are — and `gx repair` **opens anyway** and
   > reports everything else it can see.
   >
   > **Three smaller corrections.** `gx undo` on a row whose inverse body is unusable now answers
   > `INVERSE_UNAVAILABLE` — the same word the API has always used — instead of "internal error".
   > An undo that could not take the project lock back after waiting now says `busy`, instead of
   > telling you to restore a receipt file that was never damaged. And `gx commit` wrote its
   > receipt **twice**; v0.4-u's claim that the number of writes had not gone up was wrong, and it
   > is one write again.
   >
   > **What is still not closed.** The write gap of v0.4-u is unchanged and unmeasured on `git`.
   > Windows, WSL's `/mnt` and file-syncing clients are still measured **zero** times — which is
   > worth reading twice next to correction 3, because a byte-order mark and a UTF-16 save are
   > exactly what that environment produces, and this release reproduced them on Linux rather than
   > there. And a deleted inverse body is still unrecoverable: gx keeps one copy of what a change
   > replaced, and `gx checkpoint export` does not copy it.
   > (`crates/gx-cli/tests/model_a_probes.rs`, twelve tests, three of them attacks on this
   > release's own new code.)
   >
   > v0.4-w (2026-08-17, `req/38` §177, from the tenth adversarial audit `req/238`):
   >
   > **The sentence above about `gx repair` was not true of one shape, and it is the shape one
   > `rm` produces.** "A file that still will not read gets a code of its own … and `gx repair`
   > **opens anyway**" is true of a `.gx/VERSION` that is *there and broken*. Of one that is
   > **gone**, `gx repair` answered "not found", exit 6, and printed **nothing at all** — no
   > ledger, no journal, no receipts, no head, on a project where all four were perfectly
   > readable. And then it got worse: the next thing you ran that writes — `gx submit` was enough
   > — created the file again from its defaults, at exit 0, saying nothing. The project went from
   > "its declaration does not match the head it signed" back to "verified, nothing to report",
   > and no record anywhere said that the file had been missing or that gx had written it. The
   > previous release bound that file's fingerprint under the head's signature precisely so a
   > **rewritten** declaration would be caught; deleting it turned out to be the stronger attack,
   > because the next writer erased the evidence along with the problem.
   >
   > Three faces of the same hole, all measured, all with no attacker and no unusual step: a
   > `.gx/VERSION` that is **gone**; a `.gx/VERSION` that is **not text** (whatever bytes were in
   > it were thrown away and replaced, at exit 0, in silence); and a `.gx/config.toml` that is
   > gone — the file that records which key this project recovers under — which came back as the
   > shipped default, so the key you had chosen was simply no longer chosen.
   >
   > **What this release does.** No verb writes those two files back on its own any more. If they
   > are missing from a project that has a journal, every verb that writes refuses with a name for
   > it (`DECLARATION_ABSENT`, `CONFIG_ABSENT`) and a sentence telling you what to run; `gx repair`
   > **does** open, reports `declaration_absent` / `config_absent` beside everything else it can
   > see, and writes nothing at all. `gx repair --yes` is the one command that writes them back,
   > and it tells you it did, by name. If the declaration was unreadable rather than missing, your
   > bytes are moved to `VERSION.pre-repair.0` first and kept — a repair that destroyed what it
   > repaired would leave you unable to tell "gx fixed it" from "gx overwrote the evidence".
   > `gx repair --yes` will not invent the `engine_signing_keyid` line: it can put the file back,
   > and which key is yours to say.
   >
   > **Two things that were costing you.** `GET /v1/healthz` — the one endpoint that needs no
   > token — rebuilt the project's whole state table on **every** request, so a monitor polling it
   > got slower as the project grew (measured: 1.5 ms at five commits, 12 ms at four hundred, and
   > straight-line from there). It now answers from a count — **which does not make it flat**: the
   > same measurement after the change is 1.6 ms and 8.9 ms, and the rest is the check that notices
   > whether the files changed underneath a running server, which is a thing you want it to keep
   > doing. It is smaller, not free, and it is not O(1). And the test that runs three writers
   > at one project was failing intermittently under load: the writers were being correctly
   > refused with "another gx is writing, try again in 50 ms", and the test was treating the
   > correct answer as a failure. It now does what the answer says.
   >
   > **Smaller things you may notice.** `gx draft list` exists, and counts drafts whose body is
   > gone. `gx repair` publishes `head_behind_by` (a number, not a sentence) and says whether
   > `journal_intact` was checked against a chain or only against the file's length.
   > `gx log checkpoint --key` takes a key id as well as a file. `gx receipt verify` takes a
   > transformation id as well as a path, and always says `issued_at_signed: false` — the
   > timestamp on a receipt is deliberately outside the signature, and now the answer says so
   > rather than leaving it in the source. The "another gx is writing" message no longer states
   > who holds the lock as though it knew: the note in the lock file is the last one written, and
   > it can name a process that has since exited.
   >
   > **What is still not closed.** Everything under v0.4-v's own "still not closed" stands. New to
   > this list: `GET /v1/healthz` still costs more on a larger project (above), and nothing beyond
   > four hundred commits has been measured. **If you lose the key a commit was signed with, and
   > the crash window this release describes was open at that moment, that one row never
   > closes** — no command finishes it, and the remedy text will point at one that cannot.
   > Permission problems inside `.gx/` still come out as "internal error" rather than as "this
   > directory is not writable". And Windows, OneDrive and network shares are still measured
   > **zero** times. WSL's `/mnt` is no longer zero: this release ran the whole suite there once,
   > and all fifteen passed — one run, with the key store still on the Linux side.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **fifteen** tests, three of them attacks on this
   > release's own new code.)
   >
   > v0.4-x (2026-08-17, `req/38` §179, from the eleventh adversarial audit `req/240`):
   >
   > **"`gx repair --yes` is the one command that writes them back, and it tells you it did, by
   > name" was false on the road you would actually walk.** A project gx itself creates records no
   > signing key in `.gx/config.toml`. So when `.gx/VERSION` went missing and the refusal told you
   > to run `gx repair --yes`, that command hit the key check, exited 1 with "this verb needs a
   > key", and printed **nothing** — after it had already written `.gx/VERSION` and
   > `.gx/config.toml` back to disk. You read "refused"; gx had written. If you had a settings
   > line of your own in that declaration, what gx wrote was a *different* declaration, the head's
   > fingerprint never matched again, and running it a second time **with** a key reported
   > `meta_repaired: []` — no record anywhere that the file had been missing or that gx had
   > rewritten it. The same silence covered a project directory that is read-only and one where
   > another `gx` held the lock.
   >
   > **And `gx repair` called a project healthy while the next command refused it.** Delete
   > `.gx/ledger/journal` — a backup restore that skipped one file, a syncing client — and this
   > release's predecessor answered exit **0**, `remedy: null`, on a project holding two committed
   > leaves, two receipts and a signed head. It printed `ledger_leaves: 0` about a file that was
   > sitting there with two in it, because those numbers were **constants** rather than
   > measurements. The next `gx submit` refused the same project outright.
   >
   > **What this release does.** `gx repair --yes` takes the project lock, resolves the key, and
   > *then* writes — so a run that cannot sign, cannot write, or cannot take the lock leaves the
   > project byte-for-byte as it found it. A run that could not do the repair now prints the whole
   > diagnosis anyway and a line saying what it could not do and how to fix that
   > (`meta_repair_refused`) — including on a read-only copy, where the answer used to be
   > "internal error" and nothing else. A project whose journal is gone gets exit 1, the same
   > forty-seven keys every other report has, **measured** counts (two leaves, two receipts, a
   > head), and a remedy that says plainly that gx cannot rebuild a journal from a ledger. If it
   > has never held a commit, it is still exit 0 — nothing was lost.
   >
   > **Two files gx used to write behind your back.** `.gx/.gitignore` — the one file req/56 §4
   > invites you to edit — came back as the shipped default at exit 0 if you deleted it; it does
   > not any more, and `gx repair` reports it as `gitignore_absent`. And the
   > `VERSION.pre-repair.<n>` copies a repair keeps had no ceiling, no listing and no mention
   > anywhere: `gx repair` now names them (`kept_aside`), and after **eight** of them a repair
   > stops and tells you which is the oldest instead of quietly making a ninth. Nothing removes
   > one — they are the bytes that were in your file.
   >
   > **A running server now asks the same questions the CLI does.** Delete `.gx/VERSION` under a
   > live `gx serve` and the previous release went on accepting writes and answering
   > `{"status":"ok"}` while every CLI verb refused the same project. Writes now refuse with the
   > same two names (`DECLARATION_ABSENT` / `CONFIG_ABSENT`), and `/v1/healthz` answers
   > `"status": "degraded"` with a `status_reason` that names the file. It stays **200** — the
   > server is up and reads still work; what is not working is writing, and the word says so.
   >
   > **`/v1/healthz` and what it costs you.** It needs no token, and it took the engine's single
   > lock on every request, so an unauthenticated caller decided how fast everything else went.
   > It now answers from a snapshot, and the snapshot is only reused while a two-`stat` witness of
   > the journal and the ledger is unchanged — so a file rewritten under a running server is still
   > seen by the **next** probe, which is what half a dozen of this project's own tests insist on
   > (the first attempt at this used a plain 250 ms timer and turned five of them red at once).
   > The **250 ms** is a ceiling on reuse, not the detector's resolution: what it covers is a
   > change that leaves both files the same length with the same modification time. A write
   > through this server drops the snapshot immediately. **What that is measured to buy, honestly:
   > the probe itself is consistently cheaper (2.1 ms → 1.2 ms on a four-hundred-commit project),
   > and nothing else in the measurement is stable enough to claim** — the write-under-load arms
   > moved by less than they moved between two runs of the same binary. The reuse cannot help while
   > you are writing (every write changes the witness), so what it bounds is the case it can: an
   > unauthenticated flood against a project nobody is writing to. Against a client that simply
   > sends as fast as it can, the answer is a rate limit, which this release does not add.
   >
   > **Smaller things.** A repair only sweeps `.tmp` files gx itself writes; anything else you
   > left in `.gx/receipts/` or `.gx/checkpoints/` is reported and left alone. A `.gx/VERSION`
   > that declares a newer layout version than this binary understands still refuses every verb —
   > and now says what to do about it. The remedy for a missing inverse body no longer asserts
   > that a third party took it; an accident makes the same shape. The TypeScript SDK now names
   > all **twenty-one** codes this server can send (it knew thirteen), types `inverse_status` as
   > the six words it actually is, and types `retry_after_ms` — with a test that reads the Rust
   > source, so that table cannot go stale in silence again.
   >
   > **What is still not closed.** Everything under v0.4-w's own list stands except the two
   > sentences corrected above, and: the server checks that those two files **exist**, not that
   > they still read — a declaration rewritten into garbage under a live server is still caught
   > only at the next start-up. `/v1/healthz` still costs more on a bigger project, and this
   > release did not reproduce the audit's own absolute numbers (it measured 2.1 ms at four
   > hundred commits where the audit measured 8.8, because the two harnesses build the project
   > differently) — so "4.1× became 1.2×" is **not** a claim being made here. Windows, OneDrive
   > and network shares are still measured **zero** times, and WSL's `/mnt` is still the three
   > runs the previous releases made — this one added none.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **twenty** tests, five of them attacks on this
   > release's own new code — all five red on the previous binary.)
   >
   > v0.4-y (2026-08-17, `req/38` §181, from the twelfth adversarial audit `req/242`):
   >
   > **A third road wrote `.gx/VERSION`, and it was the one every ordinary command took.** v0.4-w
   > told you that `gx repair --yes` is the one command that writes that file back. It was not.
   > `gx submit` — and `gx plan`, `gx commit`, `gx undo`, and `gx serve`'s start-up — went through
   > a function that rewrote the declaration whenever it could not find a `journal_format` line in
   > it, without a word, without keeping a copy of what had been there. Two things you can actually
   > do made that visible. **One:** open `.gx/VERSION` in an editor and save it in a way that
   > leaves the first line something other than a number — `1.0`, a stray character, a re-encoding
   > that mangles it. Every command answered "this project's `.gx/VERSION` does not read as a
   > declaration", *and the next `gx submit` appended to those bytes anyway*. **Two:** delete the
   > second line by hand. `gx repair` correctly told you the declaration no longer matches the one
   > this project signed its head under — and one `gx submit` later, at exit 0 with an empty
   > stderr, the file was back to its original bytes and `gx repair` said the project was
   > perfectly healthy. The alarm and the fact that gx had silenced it were both nowhere.
   >
   > **And a lost log was replaced with an empty one.** v0.4-x's own new sentence — that a project
   > whose `.gx/ledger/journal` is gone is measured rather than called healthy — was true until you
   > ran one more command. `gx submit` created an eight-byte empty journal in its place, and from
   > then on `gx repair` reported no loss at all.
   >
   > **And `gx repair --yes` still wrote first and reported second.** v0.4-x said the write happens
   > only once the run is certain it can produce a report. The certainty was placed too early: with
   > a signing key recorded and something wrong under `.gx/ledger/` — a permission a backup tool
   > changed, a restore that put a file where a directory belongs — the command exited 1 with
   > **nothing** on standard output, and `.gx/VERSION` had been written.
   >
   > **What changed.** All of the writing of `.gx/VERSION`, `.gx/config.toml` and a new
   > `.gx/ledger/journal` now happens inside one type, in five function calls, and a test counts
   > them from the source and fails if a sixth appears anywhere in the repository. Two commands can
   > build that type: the one that turns a directory into a project, and `gx repair --yes` — which
   > cannot be built at all without the project lock and the signing key in hand. And the part of
   > `gx repair` that runs after those two cannot return an error at all, so "it wrote something
   > and told you nothing" is now a compile error rather than a thing to be found again.
   >
   > **What that costs you, plainly.** A project this release creates says which log framing it is
   > in from its first byte. A project written by an **older** gx that never recorded one **stays**
   > that way: nothing stamps it any more, `gx repair` reports `journal_format_declared: null`, and
   > the downgrade detector that the declaration powers does not apply to it. That is a real loss
   > and it is deliberate — the road that used to stamp it is the road this release closed, and
   > `gx repair --yes` will not add the line either, because doing so would change the fingerprint
   > your project's signed head was written beside. If you want that protection on an old project,
   > start a new one and re-commit into it.
   >
   > **The other things you should still do by hand.** If you edit `.gx/VERSION` yourself, save
   > `gx repair`'s output before you run anything else — this release does not rewrite the file
   > under you, but reading a report is still how you tell what changed. If `.gx/ledger/journal` is
   > gone, restore it from a backup: every command that writes refuses until you do, and `gx repair`
   > will keep telling you what the ledger, the receipts and the head still prove.
   >
   > **What is still not closed.** Everything under v0.4-x's own list stands except the three
   > sentences corrected above. Windows, OneDrive and network shares are still measured **zero**
   > times; WSL's `/mnt` is the four runs the previous releases made and this one added none. The
   > read-only-project behaviour is measured on ext4 only — on WSL's `/mnt` the operating system
   > ignores `chmod`, so that test now measures the filesystem first and says out loud that it
   > skipped. `/v1/healthz` still costs a writer more while the project is busy (the eleventh and
   > twelfth audits measured roughly the same multiple, ×4.1 and ×3.5, on different harnesses);
   > the cache helps an idle project and not a busy one. The TypeScript SDK names **twenty-two**
   > codes now, and its own test suite runs inside the release floor for the first time — before
   > this release nothing in the repository ran a line of it.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **twenty-six** tests, six of them attacks on this
   > release's own new code — five red on the previous binary, and the sixth red on this
   > release's own first build, where `gx submit` refused a directory `gx repair` called
   > perfectly ordinary — plus
   > `probes/doubt/tests/declaration_writer_doubt.rs`, **five** more that count the write roads
   > from the source and are red on the previous tree as well.)
   >
   > v0.4-z (2026-08-17, `req/38` §183, from the thirteenth adversarial audit `req/244`):
   >
   > **Three corrections to what v0.4-y told you, and they are corrections of sentences, not of
   > behaviour that changed under you.**
   >
   > **One: "it wrote something and told you nothing" was not a compile error.** v0.4-y said it
   > was. What the compiler guaranteed was that a *value* — the report — always gets made. Printing
   > it was a separate step, and printing in Rust does not report failure: it crashes the process.
   > So `gx repair --yes | head -1`, or `gx repair --yes` with standard output pointed at a full
   > disk, wrote `.gx/VERSION` and then died with exit **101** and a crash message where the error
   > object belongs — a number no table in this repository publishes, so a script could not tell
   > "gx answered" from "gx broke". And the next `gx repair` said `meta_repaired: []`: the fact that
   > gx had written a file was, at that point, nowhere at all. Every command's output now goes
   > through one road that answers for the write and the flush; a failed one is exit **1** and an
   > `OUTPUT_FAILED` error object. And `gx repair --yes` files a copy of its own report at
   > `.gx/repair/last.json`, which the next `gx repair` prints back under `previous_repair` — so a
   > run whose output you lost still leaves the fact behind.
   >
   > **Two: a project could be locked out of gx entirely.** If `.gx/config.toml` and
   > `.gx/ledger/journal` went missing together — one restore that skipped both, one `rm -rf` of
   > `.gx/ledger/` plus a settings file that never got committed — then `gx submit` refused
   > `CONFIG_ABSENT`, `gx repair --yes` did not put the settings back and its remedy did not
   > contain the word "config", and the next `gx submit` refused the same way. For ever. On a
   > project that had never recorded a commit, `gx repair` answered **exit 0** while it did that —
   > the number that means "this project can be written to". Both halves are closed: a directory
   > that has never recorded a commit gets its settings and its log made for it, exactly as it gets
   > them on the first run; and on a project that *has* committed, `gx repair --yes` writes
   > `.gx/config.toml` back and names it in the remedy. The refusal you are left with then is the
   > true one — the log is gone, restore it from a backup.
   >
   > **Three: a power cut during a commit could leave a project no repair could fix.** If the power
   > goes between the moment the ledger records a commit and the moment the log records it — a
   > window we measured at 21 ms on this machine — the two files disagree, and `gx repair --yes` is
   > what closes that. It did not, for commits made through `gx wrap`: closing the window used to
   > require re-doing the change against your files, the tool that made the change is not running
   > when you repair, and the failure to reach it was recorded as "this commit failed" —
   > permanently. Every command that writes then refused, for ever, and the remedy told you your
   > two files came from different projects, which was not true. Measured on the previous build:
   > 28 crashes inside the window, 28 projects that never became writable again. This release
   > closes the window from the receipt the commit already filed, or from the ledger entry itself,
   > and touches your files in neither case: 48 of 48 in the same sweep came back, and the next
   > `gx submit` worked. **`gx repair` now also prints `journal_behind_by`**, so "the ledger is
   > ahead of the log" is one number instead of a subtraction.
   >
   > **Smaller things this release fixed, that you may have seen.** A project whose `.gx/VERSION`
   > says its log is in the old format no longer gets a new log in the *new* format written into it
   > silently. A project that has lost its ledger, its checkpoints **and** its receipts — but still
   > holds drafts or an index — is refused with `HISTORY_LOST` instead of being handed a fresh
   > empty history over the top of what it lost; restore from a backup. A refused `gx submit` no
   > longer creates `.gx/ledger/` on its way out. `gx repair` no longer lists a file of your own
   > named `<something>.pre-repair.<n>` as something gx set aside. And a `--signing-key` that does
   > not resolve now prints the whole diagnosis instead of nothing, while keeping its own exit code.
   >
   > **What that costs you, plainly.** `.gx/repair/last.json` is a new file gx writes into your
   > project. It is a copy of a report and nothing reads it to make a decision — it is printed back
   > to you and that is all — but it is one more thing in `.gx/` and one more thing an attacker who
   > can write to `.gx/` can change or delete. If they delete it, the next `gx repair` says
   > `previous_repair: null`, which is what it also says when no repair has run. And when the
   > window above is closed from the ledger entry rather than from a receipt, **no receipt is
   > issued for that commit**: the record is complete, the proof is not. `gx repair` counts it under
   > `receipts_missing`, `gx undo` will not run on it, and
   > `gx repair --yes --reissue-receipts` from a machine that can read the files that commit
   > touched is what files one.
   >
   > **What is still not closed.** Everything under v0.4-y's list stands except the three sentences
   > corrected above and one more: v0.4-y said the read-only-project test "measures the filesystem
   > first and says out loud that it skipped". It measured the wrong filesystem — a temporary
   > directory that is always ext4 — so on WSL's `/mnt` it never skipped and simply failed. It
   > measures the project's own filesystem now, and the WSL `/mnt` run is **six** rather than four:
   > 26 of 26. Windows, OneDrive and network shares are still measured **zero** times. `/v1/healthz`
   > still costs a writer more while the project is busy, and this release did not re-measure it —
   > that is three releases in a row. Power-cut behaviour is measured on the `gx wrap` road and on
   > the plain command road (`gx submit`/`plan`/`verify`/`commit`, which the previous audit could
   > not build a harness for and which converges on both builds — 44 of 44 before, 30 of 30 after);
   > the git and postgres adapters are **not** measured. The TypeScript SDK names **twenty-four**
   > codes.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **twenty-six** tests, plus
   > `probes/doubt/tests/declaration_writer_doubt.rs`, now **seven** — the two new ones count the
   > printing roads and check that the census's own vocabulary is not narrower than the code it
   > counts. Ten self-adversarial gates were run against the previous binary and against this one:
   > **ten red before, ten green after**.)
   >
   > v0.5-a (2026-08-18, `req/38` §186, from the fourteenth adversarial audit `req/246`):
   >
   > **One correction to what v0.4-z told you, and it is the larger half of the sentence it
   > corrects.**
   >
   > v0.4-z said: "Every command's output now goes through one road that answers for the write and
   > the flush; a failed one is exit **1** and an `OUTPUT_FAILED` error object." That was true of
   > **standard output**. Errors do not go there — they go to standard error, which is where every
   > refusal this program makes is printed — and that stream was still being written by the same
   > kind of statement that crashed the process on a failed write. So a full disk, or a pipe that
   > closed first, took gx out of its own exit-code table again: `gx receipt show <an id that is not
   > there> 2>/dev/full` ended at exit **101** with a crash message. That is a **read** command, on
   > a healthy project, writing nothing anywhere. Five ways in, three runs each, exit 101 in all
   > fifteen — and the one you are most likely to write by accident is `gx repair --yes 2>&1 | head`
   > or a CI job whose disk filled up. Both streams answer for their writes now.
   >
   > **What happens when even that fails, said plainly.** If gx cannot write to standard error
   > either, it stops and gives you **the exit status it had already decided on** — 6 for "not
   > found", 1 for an error, and so on. It does not invent a new number and it does not turn your
   > refusal into a generic one, because `2>/dev/null` and `2>/dev/full` are two ways of throwing
   > the message away and a script has to get the same answer from both. What you lose in that case
   > is the message itself: the code and the explanation are gone, and the status is all that is
   > left. If the command was `gx repair --yes`, what it wrote is still in
   > `.gx/repair/last.json` and the next `gx repair` prints it back.
   >
   > **Four more things this release fixed, all of them things you could have hit.**
   >
   > **One: `gx repair --yes` used to grow its own record until it destroyed it.** Every run filed a
   > copy of its report, and the report contained the previous report, which contained the one
   > before that. On an ordinary healthy project with nothing to repair: 40 runs made a 178 KB file,
   > 126 runs made a 1.3 MB one, and run **127** could no longer read it — so it started again from
   > nothing, and "no repair has ever run here" and "126 have" became the same answer. The printed
   > report grew the same way and went past what a pipe holds, so `gx repair | head` could hand you
   > a truncated JSON object. The record keeps one report and a reference now; 130 runs in a row
   > leave it the same size.
   >
   > **Two: a project that lost its ledger, its checkpoints and its receipts was refused by
   > `gx submit` and called healthy by `gx repair`.** The refusal was right and the report said
   > "this is what `.gx/` looks like after `gx key gen` in a fresh directory" at exit **0** — about
   > a project no command could write to. Both doors ask the same question now, and the report says
   > what the refusal says: your history is gone, restore from a backup. The refusal itself has not
   > changed, and there is still no `--yes` that would invent a history.
   >
   > **Three: one file in the wrong place could lock you out of gx with no way back.** If something
   > that is not a directory ends up at `.gx/repair` — a restore, an unpacked archive — every
   > command that writes refused with "an internal error", for ever, while `gx repair` reported the
   > project healthy. It is a named refusal now (`LAYOUT_BLOCKED`), `gx repair` reports it and exits
   > 1, and `gx repair --yes` moves whatever is there to `.gx/repair.pre-repair.<n>` and makes the
   > directory. **Nothing is deleted**: gx did not write those bytes and does not remove them.
   >
   > **Four: three of the four refusals that stop `gx submit` used to create directories on their
   > way out.** v0.4-z fixed the one that had been measured. All four are checked before the first
   > directory is made now.
   >
   > **What that costs you, plainly.** The TypeScript SDK names **twenty-five** codes rather than
   > twenty-four. `.gx/repair/last.json` now keeps a pointer where it used to keep the previous
   > report in full: the previous run's report is still printed to you by the next `gx repair`, and
   > the copy on disk names the file, its size and when it was read rather than repeating it. And
   > one thing this release deliberately did **not** do: gx cannot compute a checksum of that file,
   > because the command line is not allowed to compute the identifiers this system signs (that door
   > is one crate wide, on purpose), so the pointer carries a path, a size and a time and no digest.
   >
   > **What is still not closed.** Everything under v0.4-z's list stands except the sentence
   > corrected above. Windows, OneDrive and network shares are still measured **zero** times; WSL's
   > `/mnt` is the six runs the previous release made and this one added none. `/v1/healthz` still
   > costs a writer more while the project is busy, and this release did not re-measure it — that is
   > four releases in a row. The git and postgres adapters are still not measured for power cuts.
   > And the same class of fault this release named at `.gx/repair` is still open one directory
   > down, inside the engine's own blob store (`.gx/ledger/journal.blobs/`), where a file in a
   > directory's place still answers "an internal error"; it is left to its own release rather than
   > reached into from here.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **thirty-two** tests, six of them new gates on this
   > release's findings — all six red on the previous binary and green on this one — plus
   > `probes/doubt/tests/declaration_writer_doubt.rs`, still **seven**, whose printing census now
   > counts the error stream as well as the answer stream.)
   >
   > v0.5-b (2026-08-18, `req/38` §188, from the fifteenth adversarial audit `req/259`):
   >
   > **One correction to what v0.5-a told you, and the sentence it corrects is the one in bold.**
   >
   > v0.5-a said: "**Both streams answer for their writes now.**" That was true of the object gx
   > prints when it refuses something, and it was not true of the stream. Forty-three other places
   > in the command line still wrote to standard error through a macro that cannot report a failed
   > write, and in Rust a failed write inside that macro **crashes the process**. On a full disk the
   > two that a buyer meets first were these, three runs each, no variation:
   >
   > * **`gx key gen` crashed after writing your key.** With standard error unwritable, the command
   >   ended at exit **101**, printed **nothing** on standard output, and left the secret key on the
   >   disk. The two strings that name that key — its id and its public half — were only ever on the
   >   output that never arrived, so you held a key you could not name. Both halves now survive a
   >   dead error stream: the command keeps its exit status, standard output still carries the two
   >   fields, and **`gx key list` reads both of them back out of the key store** (it gained a
   >   `public_key` field for exactly this). Nothing new is exposed — a public key is the half that
   >   is published, and the secret's location is still the only thing on standard error.
   > * **`gx wrap` crashed the same way**, and that is the membrane an agent runs behind. Its
   >   start-up line and its session summary are on standard error by design, because standard
   >   output carries the agent's protocol frames. `gx demo` stopped after step 1 of 3 for the same
   >   reason.
   >
   > The count is of **destinations** now rather than of what a line means: nothing in the command
   > line writes to standard output or standard error except one module, and a write that fails
   > there is a value rather than a crash. What a failed note costs you is stated once and it is
   > **nothing** — the command ends with the status it had already decided, because `2>/dev/null`
   > and `2>/dev/full` are two ways of throwing that stream away and a script has to get the same
   > answer from both.
   >
   > **Two: the way out of a blocked directory existed for one directory out of seven.** v0.5-a told
   > you that a file sitting where `.gx/repair` belongs is a named refusal with an exit. It named
   > the refusal for all seven directories `.gx/` declares — and the **exit** was written for
   > `.gx/repair` alone. Measured on all seven: with a file at `.gx/evidence`, `.gx/index`,
   > `.gx/drafts` or `.gx/receipts`, `gx submit` refused for ever while `gx repair` answered exit
   > **0** and "healthy", and `gx repair --yes` moved nothing. `.gx/checkpoints` exited 1 with no
   > way out. `.gx/ledger` came back as "your history is gone", which was the wrong sentence for a
   > file in a directory's place. Worse, the message gx handed you said, word for word, that
   > `gx repair --yes` would rename the path and name the copy it kept — and for six of the seven
   > that was **false**. All seven behave the same way now, the message is checked by running what
   > it tells you to run, and the shape of a declared directory is asked **before** gx asks whether
   > the project has a history.
   >
   > **What that costs you, plainly.** `gx repair`'s `repair_dir_blocked` is a **list** rather than
   > a single object, because more than one declared directory can be occupied at once. `gx key
   > list` prints one more field per key. And one honest limit on the second fix: clearing the
   > blockage restores the **shape**, not the contents — if what belonged in that directory was your
   > log or your receipts, they are still gone afterwards, and gx now says so in the same message
   > instead of letting the restored directory look like a repair. `.gx/ledger` is the row where
   > you will see that: the file is moved aside, the directory comes back, and the next `gx submit`
   > refuses by the right name.
   >
   > **What is still not closed.** Everything under v0.5-a's list stands except the sentence
   > corrected above. Windows, OneDrive and network shares are still measured **zero** times.
   > `/v1/healthz` still costs a writer more while the project is busy, and this release did not
   > re-measure it either — that is five releases in a row. The git and postgres adapters are still
   > not measured for power cuts. The same class of fault is still open one directory down, inside
   > the engine's own blob store (`.gx/ledger/journal.blobs/`), where a file in a directory's place
   > still answers "an internal error" — this release closed it across the seven directories the
   > command line owns and did not reach into the engine. And one write is deliberately outside the
   > single road: the usage error the argument parser prints when your command line does not parse,
   > which runs before there is a command to speak for; it was measured at the same exit status with
   > the error stream thrown away either way, on both binaries.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **thirty-six** tests, four of them new gates on this
   > release's findings — all four red on the previous binary and green on this one — plus
   > `probes/doubt/tests/declaration_writer_doubt.rs`, still **seven**, whose printing census now
   > counts **destinations** rather than the kind of thing a line carries.)
   >
   > v0.5-c (2026-08-18, `req/38` §192, from the sixteenth adversarial audit `req/262`):
   >
   > **One correction to what v0.5-b told you, and it is the sentence about counting.**
   >
   > v0.5-b said that "nothing in the command line writes to standard output or standard error
   > except one module", and closed with "one write is deliberately outside the single road". Both
   > were true of the command line and neither was true of **the program you install**. `gx` is one
   > binary built from fourteen crates of ours, and the count had been taken over one of them. In
   > the other thirteen there were **thirteen more** places that wrote to standard error through the
   > macro that crashes on a failed write: six in the HTTP surface and seven in the wire that
   > `gx wrap` puts an agent behind. So the honest number for v0.5-b was **fourteen** writes outside
   > the single road, not one.
   >
   > **What that cost, measured (three runs each, no variation).** With `.gx/drafts` made read-only
   > and `gx serve`'s error stream sent to a full device, `POST /v1/candidates` came back **empty —
   > no HTTP status line at all, the connection simply closed**. The same request, same project,
   > with the error stream sent to `/dev/null` instead: **201 Created**. The server stayed up and
   > answered the next request normally, so nothing monitored notices; what you lose is the
   > **answer**, and your client cannot tell "gx refused this" from "the network dropped". Neither
   > half of the fault does it alone — a dead error stream on a healthy project answered `201` both
   > ways — and both halves arrive together the day a disk fills up. Fixed: every one of the
   > fourteen is a value now, the count is taken over the whole binary, and the machine that takes
   > it computes the list of crates from the manifests rather than being handed one.
   >
   > **Two: a project the command line refuses could still be served.** With a file sitting where
   > `.gx/receipts` belongs, every `gx` verb answered exit 1 and `gx serve` **started anyway** —
   > and driving a change through that server rewrote your file, put a leaf on the ledger, and then
   > answered `500` because the receipt could not be filed. That change could not be undone
   > afterwards (the undo checks the receipt, which is not there) and could not be shown to anyone.
   > Two doors, one project, opposite answers, and the one that says yes is the one that writes.
   > Fixed: the server asks the same question the command line asks, on the way up, and refuses to
   > start with the same word.
   >
   > **Three: gx told you to run things.** The `500` above ended by telling you `gx receipt export`
   > would refile the receipt. There is no such command — `gx receipt` has `show` and `verify` — and
   > the same message called `.gx/receipts/` a directory gx can re-derive, which our own declaration
   > says it cannot ("losing this directory loses receipts"). It also named a cause it had not
   > measured: "the write permission, or the disk", where the operating system had actually said
   > `File exists`. And the message about a blocked directory told you to run `gx repair --yes`,
   > which without `--signing-key` clears nothing — measured **fourteen times out of fourteen**.
   > Fixed: the command lines gx prints are checked by **running them**, verbatim, with nothing
   > added; the message carries the flag; and where gx does not know the cause it points you at what
   > the operating system said instead of guessing.
   >
   > **What that costs you, plainly.** `gx serve` will now refuse to start on a project whose `.gx/`
   > has a file where a directory belongs, where before it started. That is stricter than v0.5-b,
   > deliberately, and the way out is printed with the refusal.
   >
   > **What is still not closed.** Everything under v0.5-b's list stands except the sentences
   > corrected above. Windows, OneDrive and network shares are still measured **zero** times.
   > `/v1/healthz` still costs a writer more while the project is busy, and this release did not
   > re-measure it either — that is six releases in a row. The git and postgres adapters are still
   > not measured for power cuts. The same class of fault is still open one directory down, inside
   > the engine's own blob store (`.gx/ledger/journal.blobs/`), where a file in a directory's place
   > still answers "an internal error". The seven writes in the wire behind `gx wrap` are repaired
   > and **not** measured through a live agent session: what was measured there is the count and the
   > type, and we say so rather than implying an arm we did not run. And the one write still outside
   > the single road is the same one v0.5-b named: the usage error the argument parser prints when
   > your command line does not parse, which runs before there is a command to speak for.
   > (`crates/gx-cli/tests/model_a_probes.rs`, **forty-one** tests, five of them new gates on this
   > release's findings — all five red on the previous binary and green on this one — plus
   > `probes/doubt/tests/declaration_writer_doubt.rs`, still **seven**, whose printing census now
   > counts every crate the `gx` binary links.)
   >
   > v0.5-d (2026-08-18, `req/38` §196 rulings DR-46-9 / DR-46-10 / DR-46-12, from `req/265`):
   >
   > **gx can now escrow a prior that a server publishes only behind a tool — and that costs you
   > requests.**
   >
   > Until this release an escrow read the thing it was about through MCP's `resources/read`. Real
   > servers often do not offer one: on `github/github-mcp-server` v1.9.0 the only resources are the
   > repository's file contents, so an issue, a pull request and a gist have **no read face at
   > all** — and an inverse gx could otherwise construct could not be escrowed, because the old
   > value was unreadable. A restore declaration may now name a **read tool** instead
   > (`"read_by": {"by_tool": ..., "arguments": {...}}`), and point into that tool's answer with a
   > JSON pointer (`{"prior_json": "/files/notes.md/content"}`).
   >
   > **What it costs, measured rather than estimated.** A guarded forward call sends **two** extra
   > tool calls to the server, not one: gx builds the inverse once when it asks the gate whether the
   > change is reversible, and again when it escrows it before applying. Both happen before the
   > effect. Against GitHub's authenticated primary limit of 5,000 requests an hour, an agent doing
   > nothing but guarded writes through a read-by-tool declaration reaches roughly **a third** of
   > the write throughput it would have unguarded. The ruling that authorised this work assumed one
   > extra read and half the throughput; the measurement says two and a third, and the measurement
   > is what this page carries.
   >
   > 🔴 **Which unit, spelled out** (added in v0.5-g, `req/291` M-06 — the numbers above are
   > unchanged and none of them is weakened). The twentieth audit re-counted the same road by
   > teeing the JSON-RPC frames and counting what **arrived at the server**, and found the sentence
   > above true in one unit and silent about the other:
   >
   > | unit | forward call | do-and-undo round trip |
   > |---|---|---|
   > | **extra tool calls** (the sentence above) | **2**, exactly — two `doc.get` | 4 |
   > | **requests reaching the server** | **10** — 7 snapshot/precondition/postcondition, 2 escrow reads, 1 effect | **21** — 19 reads and 2 effects |
   >
   > A rate limit like GitHub's 5,000 an hour counts **requests**, not tool calls. Read in tool
   > calls, a guarded write costs you a third of your unguarded throughput. Read in requests — which
   > is the unit the limit is written in — it costs you about **a tenth** (about a tenth and a half
   > over a full do-and-undo). Both numbers describe the same measured run; budget with the second.
   >
   > 🔴 **And the undo costs the same again** (added in v0.5-e, measured): an undo is itself a
   > guarded transformation, so it builds *its* inverse twice on the same road. One do-and-undo
   > round trip therefore arrives at the server as **four** escrow reads and **two** effects, not
   > two and two. The sentence above counts the forward half alone.
   >
   > **What happens when that read fails.** The effect is **refused**. Not applied and flagged —
   > refused, before the call goes out, with a message naming both ways forward. That is the
   > pre-existing behaviour made explicit rather than a new strictness, and the alternative is
   > available in writing: a catalogue that declares `"$on_read_failure": "unknown"` takes the
   > effect instead, and the reversibility of that change is **unknown** — not "false", because
   > nothing established that no inverse exists. 🔴 **Corrected in v0.5-e**: this paragraph used to
   > say "recorded as unknown", and that was one word stronger than the thing is. Unknown is
   > recorded in the refusal text and in the adapter's verdicts; the receipt payload encoding is
   > pending, and until it lands a receipt cannot tell "unknown" from "false". Even then gx does
   > not go quiet: an inverse it could not build is an escalation, so a person still has to allow
   > the change before it reaches the server.
   >
   > 🔴 **Update (2026-08-25, DR-46-21 merge, SS657 M=1 / `req/38` §421): the "is pending" sentence
   > above is stale.** The receipt payload encoding landed with DR-46-26 (`req/38` §258) — see
   > "What a receipt's read-set covers, and what it does not" below: a receipt now carries
   > `reversibility`, and `unknown` and `false` are no longer the same bytes on it. This paragraph
   > is kept as the v0.5-e historical record rather than rewritten (no-delete); read it together
   > with the corrected statement below rather than as the current state.
   >
   > 🔴 **The slot is the whole file's, not one declaration's** (added in v0.5-e). `$on_read_failure`
   > sits at the top level of a catalogue and gx reads it for **every** declaration in that file —
   > including declarations that name no read tool at all and take the `resources/read` road they
   > have always taken. One line therefore moves the fail-closed default for a whole catalogue, and
   > since v0.5-e `gx wrap` prints the value it is running under (`"on_read_failure"`, beside
   > `"restorable_tools"`) so that the relaxation is visible in operation and not only on paper.
   > 🔴 **And its reach is wider than "the read failed"** (added in v0.5-f, `req/279` M-02): this
   > slot also decides what happens when the read **succeeded** and its answer was about a
   > different object, or named no object at all. `"unknown"` turns every one of those refusals
   > into "reversibility unknown, effect taken" — so one line relaxes the object-identity binding
   > below as well as the read failure above.
   >
   > **Three things this does not do, and one of them is a footgun.**
   > (1) It does **not** make a fully tools-only server work: `snapshot` and the compare-and-set
   > still go through `resources/read`, so a server with no resources at all is still one gx refuses
   > to plan on. This release moved the escrow half of the read face and not the observation half.
   > (2) A JSON pointer is **literal** and has no variables, so `"/files/notes.md/content"` guards
   > `notes.md` and one declaration guards one member. Point it at a multi-member document and call
   > the tool against a **different** member, and gx will resolve the pointer anyway and build a
   > restore carrying the wrong member's text — while answering that the call is reversible. That is
   > declaration soundness, the same burden the catalogue's "what undoes what" already carries, and
   > it is written here because it is a way to be wrong quietly.
   > (3) Nothing marks a tool read-only that a server cannot misstate, so an operator who declares a
   > **writing** tool as a read face gets a write on every escrow. gx does not second-guess the
   > declaration; what it does is confine this road to the tool named in the catalogue with
   > arguments built from the forward call, so an agent cannot widen it.
   >
   > **What was measured and what was not.** Zero calls to github were made — every verdict below is
   > read off upstream's own tool snapshots and Go source. Of the four write tools a flag-free
   > github-mcp-server actually has, two are reversible (`create_or_update_file`, `update_gist`) and
   > two are **not**, for reasons that are mechanisms rather than effort:
   > `update_pull_request` carries `reviewers`, whose semantics are *add*, so calling the same tool
   > again does not remove what it added; `update_pull_request_branch` creates a merge commit whose
   > inverse is a force-push, and that server publishes no force-push tool. The other eleven
   > `update_*` tools live behind feature flags and were not touched. No rate limit was exercised;
   > the paragraph above is arithmetic over a measured request count.
   > (`crates/gx-adapter-mcp/tests/github16_read_by_tool.rs`, **fourteen** tests, plus one new
   > derivation in `crates/gx-adapter-mcp/tests/ac_051.rs` for the second road this opens to the
   > wire — four command-line gates for this release are red on the previous binary and green on
   > this one.)
   >
   > v0.5-e (2026-08-18, `req/38` §199 rulings 2 and 3, from the eighteenth adversarial audit
   > `req/269`):
   >
   > **The escrow v0.5-d added could be the prior of the wrong object, and on the server it was
   > built for that was the only way it ran.**
   >
   > gx watches one object and escrows another. Everything that attests a change — the snapshot,
   > the compare-and-set, the read-back after the call — reads the **locator**, the URI the change
   > names. The read-by-tool escrow v0.5-d introduced carries no locator at all: it calls the tool
   > the catalogue names, with arguments built from the agent's call, and keeps whatever came back.
   > Until this release nothing asked whether those two were the same object.
   >
   > On `github/github-mcp-server` they cannot be, by construction. A gist has no resource, so a
   > locator naming one is a locator gx refuses to plan on — measured, verbatim: *"the substrate
   > would not answer for `stdio://…#gist:g1`: this server publishes no resource at that URI"*. The
   > only deployment that runs is therefore one whose locator names some **other** object that the
   > repository does publish. **Measured consequence, three runs, no variation**: a gist was
   > changed through gx and committed; a third party then rewrote that gist; the undo ran and
   > reported success, and the third party's work was gone. The compare-and-set had been watching a
   > file nobody had touched. That is the exact accident this product is a wedge in front of, and
   > v0.5-d shipped it.
   >
   > **A second way to be wrong, from the other side.** Nothing checked the read's *answer* either,
   > so a read tool that replied with a different object's document produced "this call is
   > reversible" and an inverse carrying a stranger's text, addressed to your object.
   >
   > **What changed.** A read declaration must now say which object its answer is about, and gx
   > checks it: `"identity": ["gist:", {"answer": "/id"}]` means "the object this read answered
   > about is spelled as that resource URI", and the escrow only happens when that spelling **is**
   > the locator gx attests. Both faults above are then the same refusal from two directions, and a
   > declaration that cannot say it is a **parse error** — `gx wrap` does not start.
   > 🔴 **Corrected in v0.5-f** (`req/279` L-02): **is** overstates it. The comparison normalises
   > both sides the way every other locator on this road is normalised (RFC 3986 §6.2.2 — case of
   > the scheme, `.` and `..` segments, percent-decoding of unreserved octets), so the rule is
   > "**normalises to the same URI as**", not "is byte for byte". Measured: `file:///srv/./notes.md`,
   > `file:///srv/anything/../notes.md`, `file:///srv/anything/%2E%2E/notes.md` and
   > `FILE:///srv/notes.md` all bind to `file:///srv/notes.md`; `file:///srv%2Fnotes.md`, a trailing
   > slash and a fragment do not. That is the correct mechanism — merging normalisation is safer
   > than splitting it — and the page said something narrower than what ships.
   >
   > **A pointer that follows the call.** v0.5-d's footgun (2) — a literal JSON pointer guarding one
   > member while the call touched another, answered `true` — is closed the way it was filed:
   > `{"prior_json": ["/files/", {"forward": "filename"}, "/content"]}` builds the pointer out of the
   > call, so the member guarded is the member touched. A member the prior does not carry is now
   > `false` with an escalation instead of the wrong member's text with `true`. The plain string
   > form still parses and still means what it meant.
   >
   > **A declaration mistake now says it is one.** A read declaration that named the prior it exists
   > to produce used to be refused in the *server's* voice — "make the declared read face answer,
   > or add `"$on_read_failure": "unknown"`" — about a face that had never been called, zero
   > arrivals; and the second remedy was executable, and turned a typo into a permanent "unknown" on
   > every call. It now has its own sentence, the cause first, one remedy, and the relaxation is not
   > offered. The shapes that can be judged without a call are judged at parse time.
   >
   > **What is still not closed.** Everything under v0.5-d's list stands except the two paragraphs
   > corrected inside it (the "recorded as unknown" wording, and the cost line that counted only the
   > forward half). The compare-and-set still goes through `resources/read`, so a server with no
   > resources at all is still one gx refuses to plan on — this release bound the escrow to the
   > attested object, it did not give the attestation a second road. gx binds the **escrowed bytes**
   > to the attested object and **not the restore call's own target**: what a tool does with the
   > arguments it is handed is the server's, so a declaration whose restore template names one
   > object while its read names another is still yours to get right. `unknown` and `false` are
   > still the same bytes on a receipt. Windows, OneDrive and network shares are still measured
   > **zero** times, and zero calls were made to github.
   > (`crates/gx-adapter-mcp/tests/r17_attested_object_binding.rs`, **ten** tests, beside
   > `crates/gx-adapter-mcp/tests/github16_read_by_tool.rs`'s fourteen — nine gates for this
   > release are red on the previous binary and green on this one, four of them on the command
   > line and five in the adapter.)
   >
   > v0.5-f (2026-08-18, `req/38` §203, from the nineteenth adversarial audit `req/279`):
   >
   > **v0.5-e checked which object a read was about. It never checked that what came back was a
   > prior, that the read face was a read, or that the answer had been read at all.**
   >
   > Four of the five repairs below need no adversary. A deployment writing its own catalogue
   > reaches every one of them by hand, and the previous release answered **"this change is
   > reversible"** to three of them.
   >
   > **A read declaration with no restore template escrowed the wrong bytes.** The `arguments`
   > member of a declaration is optional and its absence means v0.1's `{contents, uri}` convention —
   > right for a restore tool that takes a resource's bytes, wrong for the tool-only road. Declare
   > `read_by` without it and the "prior contents" gx escrowed were the read tool's **answer
   > document**: measured, `verdict=true`, and an undo that left the object holding
   > `{"id":"doc:d1","text":"…","etag":"w/123"}` where its text had been. The identity check passed
   > the whole way and was right to — the read really was about that object. Nobody asked whether
   > what came back was a *prior*. The two members are now a pair, and declaring one without the
   > other is a **parse error**: `gx wrap` does not start.
   >
   > **A catalogue could name its own effect as its read face.** `{"doc.write": {…, "read_by":
   > {"by_tool": "doc.write"}}}` parsed. gx then called that tool through the escrow road — which
   > carries no admission, runs before the change is applied, and runs twice per forward call — so
   > the server was **written** under a verdict that said the effect had been refused. A file that
   > declares a tool as an effect and names it as a read face is contradicting itself in writing,
   > and that is now a parse error too. This does **not** close footgun (3) above: nothing still
   > marks a tool read-only that a server cannot misstate, and a writing tool this catalogue simply
   > never declares is still yours to get right. What closes is the case written down in one file.
   > (`"by_tool": ""` also parsed, and put an empty tool name on the wire. Also a parse error now.)
   >
   > **An `identity` could name the answer without reading it.** The parse-time check asks whether
   > an `answer` part is *present*; a member the server always answers empty satisfies that and
   > spells nothing, so the object was named by the agent's own call — and matched, of course — and
   > gx answered `true` while escrowing a stranger's text. Presence is syntax. gx now asks the
   > predicate at resolution: if replacing every `answer` part with the empty string leaves the
   > spelling unchanged, the read's answer was never checked and the effect is refused.
   >
   > **Two refusals stopped saying something false.** An answer that could not be read at all — not
   > JSON (a rate-limit HTML page is the realistic one), not UTF-8, empty, nested past the parser's
   > limit, or an `identity` pointer that resolves to nothing — used to be refused with *"the
   > declared read answered about a different object"* and handed the remedy *"point the change at
   > the object the read answers for"*. Nothing was read, so nothing named an object, so nothing
   > named a different one, and that remedy cannot be executed. Those five shapes now carry their
   > own sentence and their own remedies, and so does the `identity`-never-read case above; the
   > v0.5-e sentence is kept for the fault it was written for. Related: the shapes the eighteenth
   > audit measured as `false` (`req/269` M-02 — not JSON, nested too deep, not UTF-8, pointer
   > absent) are no longer `false`: since v0.5-e the identity gate is reached first, so under the
   > default posture they are **refusals**, and under `"$on_read_failure": "unknown"` they are
   > `unknown`. `false` on that road now means what its definition says — a declaration this call
   > carries no material for, or a body over the escrow ceiling.
   >
   > **A refusal is a sentence, not a payload.** A 1 MiB `id` produced a **1,049,118-byte** refusal
   > that travelled verbatim to the agent's tool result and to the operator's terminal. Text
   > interpolated from a server is now cut at 256 bytes with the dropped length named
   > (`...(N bytes)`); the refusal sentences themselves are unchanged.
   >
   > **What is still not closed.** Everything under v0.5-e's list stands except the two paragraphs
   > corrected inside it. gx binds the escrowed bytes to the attested object and still not the
   > **restore call's own target**. `unknown` and `false` are still the same bytes on a receipt. A
   > read face declared in one file and an effect declared in another are still two files, and gx
   > compares one. Windows, OneDrive and network shares are still measured **zero** times, and zero
   > calls were made to github. Two findings of this audit are **not** in this release because they
   > are another surface's: a change escalated from an MCP session cannot be approved on either the
   > command line or the HTTP face, and the escalation queue is not visible to a second process.
   > (`crates/gx-adapter-mcp/tests/r18_declaration_soundness.rs`, **eight** tests, beside
   > `r17_attested_object_binding.rs`'s ten and `github16_read_by_tool.rs`'s fourteen — all eight
   > gates for this release are in the adapter, seven of them red on the previous binary and green
   > on this one, and the eighth is the control they discriminate against.)
   >
   > v0.5-g (2026-08-19, `req/38` §216, from the twentieth adversarial audit `req/291`):
   >
   > **v0.5-f made a template's *absence* a parse error. A template that was there and named no
   > prior still answered "reversible" — and its undo emptied the object.**
   >
   > **A restore template built out of the forward call alone is not an inverse.** One `contents`
   > member was the whole difference. Without it the catalogue parsed, `gx wrap` started, gx
   > answered **`true`**, the gate admitted, the commit was signed, and running the undo gx itself
   > printed left the note **empty** — `rc=0`, with a signed commit receipt beside it. Every
   > fingerprint gx checks passed, correctly: they ask whether the object moved the way the applied
   > delta said, and no one asks whether it came back to where the forward call found it. A template
   > must now draw at least one member from something the forward call does not carry — the prior
   > (`"prior_contents_utf8"`, `{"prior_json": …}`) or, where the inverse is a **deletion**, the
   > applied call's own result (`{"do_result": …}`). An empty template is refused by the same gate.
   > It is a **parse error**: `gx wrap` does not start. What this does not close is unchanged from
   > v0.5-f — what a restore tool *does* with the arguments it is handed is still the server's.
   >
   > **A restore face with no name is refused too.** `{"doc.write": {"restored_by": ""}}` parsed,
   > answered `true`, and built an escrow **this same crate's decoder refuses to read back**. The
   > symmetric half of v0.5-f's `"by_tool": ""`.
   >
   > **A refusal gx makes before the gate is now recorded as a refusal in every case.** v0.5-f added
   > two refusal sentences on the same day the agent-facing surface learned to call such a stop a
   > refusal, and the two lanes did not meet: two of the five told the agent *"this is not a refusal
   > — it is a change gx could not describe"* and counted the call as a **failure of the server**.
   > The machine was right in all five (the object did not move, nothing was sent); the record was
   > wrong in two. The list is now held equal to the set of sentences by a check that reads the
   > source, so the next one added cannot be forgotten.
   >
   > **Two sentences that pointed at nothing you could do.** `gx serve`'s start-up line said an MCP
   > *ruling or undo* is refused when no server is named; `cancel` and `verify` are refused too, and
   > the line now says so. And `gx cancel` on an MCP row printed, as its remedy, the very flag that
   > same verb refuses as a usage error — so the two refusals named each other and neither could be
   > executed. Each surface now prints a remedy that exists on it: the HTTP face says to start
   > `gx serve --mcp-server …` again, and the command line says to reach the row through that face.
   >
   > 🔴 **What the template gate does not close.** It refuses a restore template that is a
   > function of the **forward call alone** — that is the shape measured to empty an object. It
   > does **not** ask whether the member a template does draw from somewhere else is the object's
   > body. A declaration that adds one constant beside its forward members passes, and if the
   > restore tool needs contents it was not given, the same destruction is reachable one member
   > wider than the shape that was measured. The gate was widened twice while this lane ran,
   > because the narrow spelling refused declarations that are sound and shipped: an inverse that
   > is a **deletion** keyed on what the forward call created, and an inverse that is a
   > **constant** (`patch-page {in_trash: false}` undoes `patch-page {in_trash: true}`). Which
   > member of a restore call carries the body is not a fact any catalogue file states, so it is
   > not a question this gate can ask. It is open, and named. Re-measured 2026-08-19 (req/322 item 7): the destructive half is **spelling-dependent** — a `{const}`-beside-forward template still admits and its undo empties the object, while the `read_by`+`identity` spelling of the same intent escalates instead (`rc=4`, object unchanged). Both measurements are true; neither closes the other. Attribution corrected 2026-08-19 (req/326 §10): what splits escalate from commit on that second bed is **not** the `identity` spelling (the two fixtures' `identity` members are byte-identical) but the type of the object the read face answers with and the number of restorable tools the catalogue declares — one restorable tool escalates (`rc=4`, nothing sent), two commit. The family's width itself is stated by the v0.5-m block below.
   >
   > **What is still not closed, and one thing this release makes plainer.** A change gx refuses
   > **before a verdict** leaves **no artifact**: no receipt, no journal record, nothing under
   > `.gx/`. The only trace is a line on the proxy's stderr, which goes when the process goes. So
   > "gx refused this call" is a thing the agent is told and **not** a thing a third party can check
   > later, unlike the three verdicts — admit, deny, escalate — which are signed and anchored. That
   > is a property of the current design and not an oversight of this lane; it is written here
   > because the surface gained that fourth outcome in v0.5-f and this page had not said what it
   > costs. Beside it, everything under v0.5-f's list stands. `gx cancel` still has no road of its
   > own to an MCP row — the repair above is to the **sentences**, and the road is the HTTP face.
   > Windows, OneDrive and network shares are still measured **zero** times, and zero calls were
   > made to github.
   > (`crates/gx-adapter-mcp/tests/r20_template_prior_soundness.rs`, **ten** tests, beside
   > `crates/gx-cli/tests/r20_undo_that_does_not_restore.rs`'s four,
   > `r20_refusal_vocabulary_is_whole.rs`'s five and `r20_mcp_surface_sentences.rs`'s four —
   > **twenty-three** gates, of which **fifteen** are red on the previous binary and green on this
   > one and **eight** are the controls they discriminate against. Those two numbers were counted
   > by running the suites against `20f0635` (`_r20_scripts/40_red.sh`), not by intending them.
   > Three of the fifteen are seams rather than measurements: they read the shipped source, so that
   > the arms beside them can hold a refusal to its wording without naming a symbol this release
   > invented — a suite that names one cannot be compiled against the old source at all, and its
   > red would be a missing symbol rather than the defect.)

   >
   > v0.5-h (2026-08-19, `req/38` §218, DR-46-16 — the compare-and-set half of `req/38` §123
   > ruling 1 (b)):
   >
   > **A tools-only server can now be planned on — for the objects a deployment names, and for no
   > others.**
   >
   > Until this release, everything above about the read-by-tool road was about the **escrow**: the
   > prior gx keeps so that an undo has something to restore. The other half — the compare-and-set,
   > which is what makes an undo refuse when somebody else moved the object — still went through
   > MCP's `resources/read` unconditionally. A server that publishes no resource face for an object
   > therefore had no compare-and-set at all, and gx refused to plan on it however complete its
   > restore catalogue was. That was written on this page and in the code, and it was the honest
   > shape: no read, no attestation, no undo worth signing.
   >
   > A restore catalogue may now carry a third reserved slot, `"$cas_read"`, mapping a **resource
   > URI prefix** to the read tool that answers for objects under it:
   >
   > ```json
   > "$cas_read": {
   >   "notion://page/": {
   >     "by_tool": "API-retrieve-a-page",
   >     "arguments": { "page_id": "resource_suffix" }
   >   }
   > }
   > ```
   >
   > When a prefix matches, the three reads gx makes about an object — the snapshot it plans
   > against, the fingerprint it commits against, and the read-back after the call — all go through
   > that tool. When none matches, all three go through `resources/read` exactly as before. The
   > longest matching prefix decides. The arguments are built from the locator and from constants
   > the declaration supplies, and from nothing else: at the point gx takes a snapshot there is no
   > agent payload in existence for anything else to come from.
   >
   > 🔴 **What this does not close, and it is the first thing to read here.**
   >
   > **A `$cas_read` declaration is not checked against the object it claims to be about.** The
   > escrow half's read declaration carries a required `identity` — the deployment spells how the
   > read's own answer names the object, and gx refuses when that spelling is not this object's
   > (v0.5-e above, after an audit measured an undo overwriting a third party's write). The
   > compare-and-set half's declaration carries **no such member yet**. So a `$cas_read` tool that
   > answers about a *different* object produces this object's digest out of that object's bytes,
   > and gx will not notice. The same predicate is owed here and is the next lane's (`req/38` §218
   > ruling 2, DR-46-21). Until it lands, `"$cas_read"` is a deployment's **word** that the tool it
   > names answers for the locators it is keyed by — treat it with the care the rest of a restore
   > catalogue already asks for, and prefer a read tool whose argument is derived from the locator
   > itself, as the example above does.
   >
   > 🔴 **Update (DR-46-21, `req/38` §417/§421): the content-addressed case is now closed; the
   > name-keyed case above is not, and that is the first thing to keep reading here.** A `$cas_read`
   > whose locator is keyed by the object's **digest** — the suffix after its matched prefix is a
   > `gx1:` CID — is now re-verified: the bytes the declared read answers must hash to that CID, and
   > a mismatch is refused fail-closed, so the dishonest server that answers a neighbour's bytes no
   > longer produces this object's digest out of a stranger's. It is opt-in **by construction** and
   > there is no new member to spell: the check fires only when the value keying the read is itself a
   > CID, so a name-keyed `$cas_read` (`notion://page/abc-123`, whose key is not a digest) is
   > returned unchanged and every name-keyed declaration is byte-for-byte where it was. What that
   > leaves open is said plainly rather than smoothed over: a **name-keyed** read has no digest to
   > check the answer against, so a dishonest server answering about a different object under a
   > name-keyed prefix — the `doc://pageant` shape in v0.5-i below is one — is **still not caught**,
   > and the paragraph above holds for it unchanged. Binding a name-keyed read would take a different
   > predicate (a call-target invariant, not a digest) and it is not in this build.
   >
   > **Declaration unlocks one prefix and nothing else.** An object on a tools-only server that no
   > `$cas_read` pattern matches is still refused at plan time. That refusal is not degraded into a
   > silent unattested commit, and it is not a bug to report: it is the same fail-closed answer this
   > page has described since v0.4, reached one road later.
   >
   > **The escrow half is still a separate declaration.** `"$cas_read"` unlocks the compare-and-set;
   > `"read_by"` (v0.5-d) unlocks the escrow, and a tools-only deployment that wants both writes
   > both. They were deliberately **not** merged: the escrow declaration carries the identity check
   > the compare-and-set declaration does not yet have, and letting one stand in for the other would
   > have handed the escrow a source of bytes nothing binds to the object — the exact accident
   > v0.5-e closed.
   >
   > **A tool named as a compare-and-set read face may not also be an effect this catalogue
   > undoes.** gx calls a `$cas_read` tool from the snapshot, which happens before the change is
   > planned and carries no admission, so a file that says both is asking gx to make an unadmitted
   > change while it is working out what the object currently holds. It is a parse error, as the
   > escrow half's version of the same contradiction has been since v0.5-f. What is **not** closed
   > is the general case, unchanged from v0.5-d: nothing in MCP marks a tool read-only that a server
   > cannot misstate, so declaring a *writing* tool as a read face — one this catalogue does not
   > also declare as an effect — is a burden the deployment carries.
   >
   > **No round trips were added.** Each of the three reads costs one call on either road; the
   > declaration changes which face the call speaks to, not how many are made. Nothing measured here
   > touched notion.so, github.com or any network: the servers in these tests run in the same
   > process, and Windows, OneDrive and network shares are still measured **zero** times.
   > (`crates/gx-adapter-mcp/tests/dr46_16_cas_read_by_tool.rs`, **fourteen** tests, beside the two
   > added to `notion_page_catalogue.rs` — one holding that an undeclared tools-only page is still
   > refused, one driving the declared road end to end on the server shape that motivated the
   > ruling — and the second road added to `ac_051.rs`'s D-6 derivation, which now counts and names
   > **two** paths to a tool call and bounds each by what its declaration may send.)

   >
   > v0.5-i (2026-08-19, `req/38` §221, from the twenty-first adversarial audit `req/303`):
   >
   > 🔴 **Corrected, not deleted — v0.5-g told you something about this product that is false.**
   > The paragraph above under v0.5-g reads, verbatim:
   >
   > > A change gx refuses **before a verdict** leaves **no artifact**: no receipt, no journal
   > > record, nothing under `.gx/`. The only trace is a line on the proxy's stderr, which goes when
   > > the process goes. So "gx refused this call" is a thing the agent is told and **not** a thing a
   > > third party can check later.
   >
   > One third of that is true. The rest was written from one road and published as if it were the
   > product. Both roads have now been driven and counted, in the same project, with the whole of
   > `.gx/` censused by path and byte length before and after
   > (`crates/gx-cli/tests/r22_wrap_road.rs::the_fourth_outcome_leaves_what_it_leaves_on_each_road`):
   >
   > | the road | what the agent is told | journal records | files under `.gx/` | receipts |
   > |---|---|---|---|---|
   > | the proxy answers **before it submits** — a call naming no resource, refused by `gx wrap` itself | `"gx/verdict": "not-planned"` | **0** added | **0** new, **0** changed | **0** |
   > | the deployment's **declaration** is refused when it is resolved, after submit/plan/verify have each run | `"gx/verdict": "refused-before-verdict"` | **3** added: `DraftCreated`, `Planned`, `VerifyStarted` | **2** new (`drafts/<id>.json`, one journal blob), **3** changed (`LOCK`, `ledger/journal`, `index/intent_to_transformation.json`) | **0** |
   >
   > **What is true on both roads: no receipt is written.** That half of the v0.5-g sentence stands,
   > and it is the half that matters most, because a receipt is the thing this product signs.
   >
   > **What is false: "no journal record, nothing under `.gx/`".** On the second road a third party
   > reading the journal finds a transformation that reached `Planned` and `VerifyStarted` and
   > carries **no verdict record** — a durable, inspectable trace that gx stopped. That is more than
   > v0.5-g claimed, not less, so the correction is in the safe direction; it is published anyway,
   > because a limits page that under-claims is still a limits page that is wrong, and the sentence
   > was written as this project's *honest declaration* of a gap.
   >
   > **What is still open, stated exactly.** That trace is an *absence* — the reader infers the stop
   > from a missing verdict record — and it is **not attested**: nothing signs it, and nothing on a
   > machine with no copy of the project can check it. The gap DR-46-20 is filed for is therefore
   > narrower than v0.5-g made it sound, and it is real: **a refusal before a verdict has no
   > receipt-grade attestation**. It is not the case that it leaves nothing behind.
   >
   > 🔴 **And the session line counts it in its own bucket now.** v0.5-g called the proxy's stderr
   > line "the only trace"; that line spent this outcome as `denied: 1`, in a counter whose own
   > definition is *how many the gate denied* — and the gate never saw the call. `gx wrap`'s session
   > line carries an eighth field, `refused_before_verdict`, printed as a zero when there are none.
   > `denied` and `failed` keep the meanings they are documented with.
   >
   > 🔴 **The gate on this page is now a measurement.** The paragraph v0.5-g published was held by a
   > probe that searched `docs/LIMITS.md` for the sentence's own words. It found them every time,
   > for three lanes, while the sentence was false. The probe that holds this table counts journal
   > records on a real road and requires the page to name every kind it measured
   > (`req/38` §221 ruling 3).
   >
   > 🔴 **A restore template of forward members and a hash of one of them was still an inverse to
   > gx.** v0.5-g's gate refuses a template that is "a function of the forward call alone", and it
   > was implemented as "at least one member is not `{"forward": …}`". `{"git_blob_sha1_of_forward":
   > "contents"}` is not `{"forward": …}` and is, by construction, computable from the forward call
   > before it is made — so `{"uri": {"forward": "uri"}, "sha": {"git_blob_sha1_of_forward":
   > "contents"}}` passed. Measured: the catalogue parsed, gx answered **`true`**, the commit was
   > signed, and the undo gx printed emptied the object with `rc=0` — the same destruction v0.5-g
   > announced as closed, one word wider. The gate now asks which words draw on something the
   > forward call does not carry, by name, one arm per word; a word added to the vocabulary later
   > and not classified is treated as *carried by the forward call*, so it cannot satisfy the gate
   > on its own. **What is still not closed is unchanged and is the same sentence v0.5-g ended on**:
   > a single `{"const": …}` member satisfies this gate while carrying nothing of the prior, and
   > which member of a restore call is the object's body is not a fact any catalogue file states.
   > That is an **accepted residual**, in those words, rather than a gap awaiting a lane
   > (`req/38` §221 ruling 2).
   >
   > 🔴 **Two declarations that render identically are now a parse error.** `écrire` written with
   > U+00E9 and `écrire` written as `e` + U+0301 are one name to a reader and two keys to a parser,
   > and a catalogue file is approved by reading. A tool name carrying a combining diacritical mark
   > is refused with the composed spelling named as the fix. **The width is exactly this**: the five
   > Unicode combining-diacritical blocks. Hangul jamo, Hebrew points, Arabic harakat, Indic matras,
   > singleton equivalences (U+212B against U+00C5) and compatibility (NFKC) confusables are **not**
   > covered, and a right-to-left override (U+202E) or a zero-width space (U+200B) inside a tool
   > name is still accepted. Closing the rest needs a Unicode normalisation table this crate does
   > not carry.
   >
   > 🔴 **A catalogue file has no size ceiling, and that is not an oversight this page had
   > mentioned.** `MAX_FORWARD_PAYLOAD_BYTES` (1,048,576) bounds the *payload of a call*; nothing
   > bounds the restore catalogue itself. The twenty-first audit parsed a **1,596,001-byte file
   > declaring 12,000 tools** and it was accepted. Template nesting is bounded — `serde_json`'s
   > recursion limit refuses at depth 127 and a 100,000-deep spelling does not abort — but the file
   > as a whole is not. The file is named by the operator at start-up, not by an agent, so this is a
   > property of the format rather than an attack surface; it is written down because it was not.
   >
   > 🔴 **How the throughput table above was measured, so the numbers can be reproduced or
   > disagreed with.** The independent re-count in `req/303` L-01 matched the forward-call column
   > exactly — 10 requests, with the same 7/2/1 breakdown — and matched the extra-tool-call column
   > exactly, and got **23** where the round-trip column says 21. The difference is two reads on the
   > undo leg. The setup behind the **21**: one `gx wrap` session over a document-store server with
   > a `read_by` declaration, one forward `doc.write`, then the undo driven **inside the same
   > session**. The re-count drove the undo as a **separate `gx undo --mcp-server …` process**,
   > which opens its own session and reads twice more before it applies. Both numbers are of real
   > runs; they are of different setups, and the difference is the second session's own escrow. If
   > you are budgeting for an agent that undoes from a fresh process — which is what `gx wrap`'s
   > printed `gx undo …` remedy does — **23 is the number to use**.
   >
   > 🔴 **A declaration fault no longer wears another fault's face.** Every entry fault used to be
   > printed as *"the read declaration of entry X is not sound"*, and on the road a proxy reports,
   > *"The declared read face was never called"* was appended to it — over catalogues that declare
   > no read face at all, with a remedy (*correct the read declaration*) naming nothing the reader
   > could open. Faults now name the half of the declaration they are about: the restore
   > declaration, the `arguments` template, or the read declaration.
   >
   > 🔴 **`--restore <TOOL>=<RESTORE_TOOL>` is checked at start-up like the file is.** v0.5-f said of
   > a blank restore name *"It is a **parse error**: `gx wrap` does not start"*. That was true of
   > `--restore-catalogue <file>` and false of the flag, which built the entry past the only reader
   > that ran the check; the session started, the server was spawned, and the same declaration was
   > refused per call instead. Both mouths now refuse at start-up, in one wording.
   >
   > 🔴 **`gx wrap`'s start-up line says how many `$cas_read` declarations it read.** DR-46-16's slot
   > changes the road every locator under a declared prefix is read by, and until now the line
   > printed `restorable_tools` and `on_read_failure` and nothing about it. A zero is printed when
   > there are none.
   > (`crates/gx-adapter-mcp/tests/r22_declaration_gates.rs`, **fifteen** tests, beside
   > `crates/gx-cli/tests/r22_wrap_road.rs`'s eight, `r22_serverless_surface.rs`'s three and
   > `r22_refusal_constant_census.rs`'s four. Windows, OneDrive and network shares are still
   > measured **zero** times, and zero calls were made to github or notion.)

   >
   > v0.5-j (2026-08-19, `req/38` §225, from the twenty-second adversarial audit `req/312`):
   >
   > 🔴 **gx told an agent that nothing had been sent to the server while the server's own log held
   > two frames gx had sent — and the document was empty.** This is the worst thing on this page and
   > it is written first. The gate that decides whether a `$cas_read` face may be called asked one
   > question: *is this tool one this catalogue declares as an effect?* A catalogue file names the
   > tools it writes with in **two** places, and the second one is `restored_by` — the call that
   > puts an object back, which writes by construction and which the file itself says so about. A
   > deployment that followed the remedy the first gate prints, **verbatim** (*"name a read face
   > this catalogue does not declare as an effect"*), arrived at a declaration gx accepted, and gx
   > then called that restore tool from `snapshot` — before the transformation had a plan, before
   > any gate saw it. Measured on a real server on both roads: on the admitting road, six arrivals
   > before the agent's own call, a signed commit, and an undo that did not recover the document
   > (`rc=4`); on a road where gx **refused before a verdict** — zero receipts, the agent's call
   > never framed — the document was destroyed anyway and the agent was handed *"gx refused this
   > call and **nothing was sent to the server**"*.
   >
   > Two repairs, and they are independent:
   >
   > 1. The soundness gate now asks about **both** sets a catalogue file declares — the tools it
   >    calls effects and the tools it calls inverses. Such a file is a start-up error and names
   >    which of the two it fell into.
   > 2. Every outcome of `gx wrap` that did not send the effect now says **what it did send**:
   >    *"the effect was not sent, and N reads were sent to the server to establish what the object
   >    holds"*, counted for that call alone. When gx really sent nothing, it still says so.
   >
   > 🔴 **What is unchanged, and is the reason the second repair is not redundant.** A read face
   > that is a **third party's** writing tool — one this catalogue names neither as an effect nor as
   > an inverse — is still a burden the deployment carries, exactly as v0.5-h said: nothing in MCP
   > marks a tool read-only that a server cannot misstate. gx cannot see that from the file. What it
   > can now do is tell the agent how many reads it sent, which is the number that makes such a
   > burden landing observable from the answer rather than from the object.
   >
   > 🔴 **A receipt could say an object was empty when the read had merely failed.** After a call is
   > made, gx reads the object back and signs that as the postcondition. `Unreadable` is the
   > transport's word for two different facts — *there is nothing here* and *I could not tell you* —
   > and the read-back folded both to the digest of no content. Measured: an object holding 24 bytes,
   > a signed postcondition of absence, and every later `gx undo` of that receipt refused
   > `PRECONDITION_CHANGED` — *the world moved after the transformation being undone committed* —
   > over a world that had not moved. The invariant now holds in one line:
   > **a read that failed is never signed as absence.**
   > It is refused, the effect is stated as not in doubt, and the remedy
   > is to make the read face answer and run the transformation again (the call log makes that a
   > retry rather than a second effect). `absent_digest` is reached only where the server
   > **answered** that the locator holds nothing — which on MCP means the resource-not-found code,
   > and this repository's two servers now send it. A transport that cannot tell the two apart gets
   > the fail-closed reading, because saying nothing is not saying "there is nothing there".
   >
   > This defect was **not** DR-46-16's: the audit drove the same three configurations through the
   > older `resources/read` road and got a bit-identical postcondition and a bit-identical undo
   > refusal. DR-46-16 widened what `Unreadable` could mean without widening the fold's argument.
   >
   > 🔴 **The combining-mark gate ran in one of the five places a name is written.** v0.5-i says of
   > it *"the width is exactly this"*, and the width it described is which marks are covered. There
   > is a second axis: **where** the question is asked. It was asked of the catalogue's keys, and
   > `restored_by`, `read_by.by_tool`, the `$cas_read` prefix and the `$cas_read` `by_tool` all
   > accepted the decomposed spelling — so the harm v0.5-i closed (two declarations an operator
   > cannot tell apart while approving them) reproduced in four positions, including one where two
   > prefixes rendering as one line became two reading roads and a byte the page does not show
   > decided which governed an object. The gate is now asked in all five. The **mark** width is
   > unchanged and so is everything v0.5-i says it does not cover.
   >
   > 🔴 **A `$cas_read` prefix was compared as written against resource URIs that are always
   > normalised.** `Doc://Host/Page/` parsed, was counted on the start-up line, and unlocked
   > nothing. Prefixes are now normalised the way a resource URI is; two spellings that normalise to
   > one are a **parse error** rather than one silently winning.
   >
   > 🔴 **And the prefix was a byte prefix.** `doc://page` governed `doc://pageant/secret`, which is
   > a different name space read through this declaration's tool — and because a CAS read's answer
   > is not bound to the object it is about for a **name-keyed** prefix like this one (DR-46-21 now
   > binds a content-addressed locator, `req/38` §417/§421, but not this shape), the neighbour's
   > bytes would become this object's digest. A prefix now has to end where a segment ends: on `/`,
   > on `:`, at
   > the whole URI, or immediately before a `/` in the resource. `doc://` still governs every
   > `doc://…`, which is the shape a deployment writes.
   >
   > 🔴 **`gx replay <a transformation this project has never held>` answered `matches: true` with
   > exit 0.** `records_replayed: 0` was printed beside it, and a person reading the object can see
   > it; a tool branching on `matches` cannot. Every other verb of this binary that takes an id —
   > eleven of them were measured — answers `NOT_FOUND` and exit 6 to the same argument, so this was
   > not merely a vacuous pass: one verb disagreed with the rest of the binary about what the id
   > names. It now answers `NOT_FOUND` and exit 6 as well. A replay of an id the journal does hold,
   > and a replay of the whole journal, are unchanged.
   >
   > 🔴 **The two faces of this binary named one refusal two ways.** `gx-api` has answered
   > `gx_engine::Error::NotFound` with `NOT_FOUND` (declared exit **6**) since M6-09; the CLI
   > carried it to `INTERNAL` and exit **1** — 44 §2.3's word for what *cannot be classified*, over
   > a refusal that is completely classified. The word and the number now move together, and only
   > for the two subjects a caller actually names (a transformation, a draft). *"No adapter is
   > registered for this substrate"* — nine of the engine's twenty-one such sites — is a statement
   > about something nobody named and stays `INTERNAL`; the fold is declared on the HTTP face rather
   > than left to be discovered. **No exit status a script has seen has moved**: eleven id-taking
   > verbs were put against well-formed absent ids and the engine road was produced zero times, and
   > that sweep is now a probe rather than a sentence in a report.
   >
   > 🔴 **A correction to v0.5-i's own table.** The row for the second road of the fourth outcome
   > says *"2 new, 3 changed"*. That split is a function of the **project's prior state**, not of
   > the road: in a project whose `index/intent_to_transformation.json` does not exist yet the same
   > road leaves 3 new and 2 changed. Five files are touched either way, the journal record kinds
   > are the same three, and the receipt count is 0 on both roads. Read the table as *five files,
   > and which of them are new depends on what your project already had*.
   > (`crates/gx-adapter-mcp/tests/r23_cas_declaration_gates.rs`, **fourteen** tests, beside
   > `crates/gx-cli/tests/r23_wrap_road.rs`'s five and `r23_not_found_road.rs`'s six.
   > Windows, OneDrive and network shares are still measured **zero** times, and zero calls were
   > made to github or notion.)

   >
   > v0.5-k (2026-08-19, `req/38` §227, from the twenty-third adversarial audit `req/316`):
   >
   > 🔴 **The repair v0.5-j described landed on one of two gates, and the other one emptied a
   > paragraph a person had written.** v0.5-j said *"The soundness gate now asks about **both** sets
   > a catalogue file declares — the tools it calls effects and the tools it calls inverses"*. It is
   > written in the singular and there were two gates: the `$cas_read` face's, which was repaired,
   > and the escrow road's `read_by` face, which asks the identical question and still asked only
   > about effects. A deployment that followed the remedy that second gate prints, **verbatim**
   > (*"name a read face this catalogue does not declare as an effect"*), arrived at a `read_by`
   > naming the file's own `restored_by` — and gx called that restore tool from the escrow, before
   > any verdict existed. Measured on a real server: the document went from a paragraph to empty,
   > the agent's own call was **never sent** (`client_calls: 0`), no transformation id was minted
   > and no receipt was signed.
   >
   > What is different this time is that the repair is not a third gate spelled correctly. There is
   > now **one** function that answers "does this file, in what it writes down, say that this tool
   > writes", and every gate calls it; a `catalogue.rs` in which that question is spelled anywhere
   > else fails the build. Three consecutive audits found one instance each of the same shape, which
   > is what turned it from a defect into a rule.
   >
   > 🔴 **A server could forge an absence by writing gx's own sentence into its error message.** The
   > decision "the server answered that this locator holds nothing" is a JSON-RPC code equality —
   > `-32002` and nothing else — but the way that decision crossed into the adapter was a token in a
   > free-text field that also carried the server's own message, tested with a substring search. A
   > server answering `-32603` (*I could not tell you*) with those 74 characters in its message got
   > a signed postcondition of absence over an object still holding its bytes, with a fingerprint
   > **bit-identical** to a genuine absence — which is the defect v0.5-j closed, returning through
   > the middle of the repair that closed it. The token now has a fixed **position** rather than a
   > wider search: gx writes it first, and the far side asks for it at position 0. A third-party
   > transport that composes it any other way loses absence detection, which is fail-closed and is
   > written down here because it is otherwise silent.
   >
   > 🔴 **A server with no `resources/read` face could not record a deletion at all.** `$cas_read`
   > exists for tools-only servers, and the road it reads by never wrote the absence marker. So on
   > exactly the substrate class that slot was built for, a call that **removed** a resource was
   > refused fail-closed under every wiring, and the refusal printed a remedy — *make the read face
   > answer for this locator and run it again* — that cannot be executed against an object that is
   > gone. The declared road now carries the same answer the `resources/read` road does, by the same
   > code equality and nothing wider: an `isError` result and an answer with no content are still
   > "I could not tell you".
   >
   > 🔴 **`gx wrap --record-only` printed that it was in record-only mode and then behaved exactly
   > as a run without the flag.** 43 §4 is unambiguous: under record-only a `Denied` transformation
   > advances through T-8r to `Committed` and the receipt must always carry `enforced=false`. A
   > `Deny` is the only verdict the flag means anything on, and on that road `gx wrap` returned
   > before canonicalisation: same verdict, `enforced: true`, zero arrivals at the server, the object
   > untouched. Both halves now hold — the effect is sent and the receipt says policy did not
   > enforce it — and the **verdict itself does not move**, because a receipt saying `Admit` here
   > would destroy the fact the mode exists to preserve. The enforcing default is unchanged: without
   > the flag, a denied call still never reaches the server.
   >
   > **A correction to a note this repository has carried since M6H3-9.** That note says a
   > record-only end-to-end run needs a policy pack over a writable path and that `gx verify` has no
   > `--policy`, so it could not be built. `gx wrap` **has** `--policy`, and the suites already ship
   > a pack fixture over a writable path. The blocker does not hold on this surface, and it is
   > recorded here rather than left standing.
   >
   > 🔴 **When a change lands and the automatic roll-back also fails, the ledger says only that the
   > apply failed.** 43 T-10c rolls back from the escrowed inverse on a **best-effort** basis, and
   > the record it writes is `Aborted{ApplyFailed}` whether that effort worked or not. Measured on a
   > real server: the write arrived, the compensating restore arrived and was refused, the object
   > kept the change, and no transformation committed — so there is no `gx undo` for it either. The
   > record is the engine's and it has not moved; what has changed is that the sentence the agent
   > receives now says the call was sent, that the change may stand, that the compensation was
   > best-effort with its own outcome in the record beside it, and that no committed transformation
   > exists to undo. A call that committed carries none of that.
   >
   > 🔴 **Two spellings a catalogue file could carry that nobody meant.** A declaration written as a
   > JSON **array** parsed, because the deserialiser reads a struct as a sequence as well as a map —
   > so the slots were decided by field order in a Rust struct the operator approving the file cannot
   > see, and a `$cas_read` written that way reached the real read road. It is now a start-up error.
   > And a tool name with **whitespace at either end** was a second name: the sets this file draws
   > about itself are compared by their bytes, so a read face spelled with a trailing space walked
   > past the gate that refuses the same tool spelled without one. That is now a start-up error in
   > all four places a file spells a tool name.
   >
   > **What that second repair does not cover, exactly.** It closes the whitespace axis. Two
   > spellings that are **canonically equivalent** under Unicode without being equal as bytes — the
   > angstrom sign against the letter A with a ring — are still two names to these sets, and closing
   > that needs a normalisation table this build does not carry. It is filed rather than fixed, and a
   > probe holds it open so that the day it closes, this paragraph moves in the same commit.
   > (`crates/gx-adapter-mcp/tests/r24_predicate_unification.rs`, **twelve** tests, beside
   > `r24_absence_discrimination.rs`'s eight and `crates/gx-cli/tests/r24_record_only_and_removal.rs`'s
   > seven. Windows, OneDrive and network shares are still measured **zero** times, and zero calls
   > were made to github or notion.)

   >
   > v0.5-l (2026-08-19, `req/38` §229, from the twenty-fourth adversarial audit `req/320`):
   >
   > 🔴 **gx told an agent it had tried to undo a change it had never tried to undo.** When an
   > admitted call lands and the post-apply read then fails, 43 T-10c rolls back from the escrowed
   > inverse on a best-effort basis — and v0.5-k added one sentence saying so to **every**
   > `ApplyFailed`. The roll-back has three outcomes, not two. The third is *not attempted*, which is
   > what happens when the escrowed inverse is still partial: a member of it is filled from what the
   > call answers, and a failed apply leaves no answer to fill it from, so nothing is sent. Measured
   > on a real server: the write arrived, the object took the agent's bytes, the server's own arrival
   > log held **one** line, and gx said the compensating inverse *was attempted*. The sentence is now
   > one arm per outcome and a word this build does not recognise gets an arm of its own, so the
   > claim can never again be wider than what happened.
   >
   > **And what the recorded outcome means, exactly.** `Failed` is the compensating *apply* returning
   > an error, and this adapter's apply is the call **together with** the read-back of it — so a
   > compensation whose bytes landed and whose read-back died is recorded `Failed` with the object
   > back where it started. It was being offered as the answer to *did the compensation work*. It is
   > the answer to *did the compensating apply return successfully*, and the sentence now says which.
   >
   > 🔴 **`gx wrap --record-only` said policy had been enforced over a call it had let through.**
   > v0.5-k reported both halves of 43 §4 holding, and it measured the road where the commit
   > succeeds. On the road where the apply then **fails**, an aborted transformation writes no
   > `enforced` member at all, so the value handed to the agent fell back to the one `verify` wrote
   > before T-8r had run: `gx/enforced: true`, over a `Deny` that record-only had carried past the
   > gate and sent. The same answer's sentence opened *"gx admitted this call"* while `gx/verdict`
   > beside it said `Deny`. Both are repaired: the flag is `false` on that road because T-8r is the
   > only way a `Deny` reaches a commit, and the sentence names the verdict the gate reached.
   >
   > **What `--record-only` does not do, said rather than left to be measured.** It does not reach an
   > **escalation**. 43 §4 is written about a `Denied` transformation, and an escalation is not a
   > refusal a person has made — it is a question nobody has answered yet, so carrying it through
   > would send an effect no policy admitted and no person ruled on. The flag was already inert
   > there; what is new is that the answer says so instead of leaving an operator to run it twice and
   > compare.
   >
   > 🔴 **After a removal gx itself admitted, the next call was refused in words that denied their own
   > evidence.** The discriminator that separates *the server answered that this locator holds
   > nothing* from *the server would not tell me* was asked at **one** of the three places gx
   > consumes a read; the other two are `snapshot` and `precondition`. So one sentence carried both
   > `gx-substrate`'s *the substrate would not answer* — a frozen face this build may not reword —
   > and gx's own token saying the server had answered. The refusal is unchanged and is still
   > fail-closed: gx mediates a change to an object that exists, and there is no prior state for a
   > compare-and-set to be conditional on. What is new is that it says which of the two facts stopped
   > it, and names a remedy that can actually be executed.
   >
   > 🔴 **A correction to what v0.5-k claimed about its own gate.** That block said *"a `catalogue.rs`
   > in which that question is spelled anywhere else fails the build"*. The gate counted two
   > **strings**, and a third gate written `restores.get(t).is_some()` beside `tool ==
   > s.restored_by()` asks the identical question in neither of them: the build stayed green. The
   > gate now counts every read of the private map itself, with the function each one sits in, so a
   > gate asking that question in **any** spelling has to appear in a list or be red — and a probe
   > fires the equivalent spelling at it rather than describing it. The sibling half is measured too:
   > no other file in the crate reaches that map, which Rust already guarantees and nothing had
   > checked.
   >
   > 🔴 **The invisible-edge gate walked four of the five positions, and its axis was not the axis it
   > declared.** v0.5-k closed *whitespace at either end* in the four positions a **tool name** is
   > spelled. A declaration spells a name in **five**: the fifth is the `$cas_read` prefix, and it is
   > the quietest of them — a prefix with an invisible edge matches no locator, so the declared
   > tools-only read road is silently not taken and every read falls back to `resources/read` on a
   > deployment that believes it opted in. It is now a start-up error like the other four. **And the
   > axis is now enumerated rather than described**: the gate's own words are *an edge a reader
   > cannot see*, and it was implemented as `char::is_whitespace`, which answers `false` for U+200B
   > and U+FEFF — both accepted in all five positions when the audit fired them. The width is
   > `char::is_whitespace` **plus exactly five scalars**: U+200B, U+200C, U+200D, U+2060, U+FEFF.
   > That corrects v0.5-i's *"a zero-width space (U+200B) inside a tool name is still accepted"* at
   > one end only: inside a name it still is, at either **edge** it is not. The three *unnamed* gates
   > were widened in the same commit and to the same predicate, because a name of only zero-width
   > scalars would otherwise have been neither unnamed nor edged. Canonical equivalence (the angstrom
   > sign against A-with-ring) and a right-to-left override are **still** open, still need a
   > normalisation table this build does not carry, and a probe still holds each open.
   >
   > 🔴 **The accepted residual was re-measured and it is not narrower.** The twenty-fourth audit,
   > driving a *different* declaration, found the printed undo refusing with `rc=4` and the object
   > unmoved, which read as though the residual had closed. It has not. On this build, `req/312`'s
   > own spelling — a restore template whose only member drawing on anything the forward call does
   > not carry is a `{"const": …}` — is accepted, admits, commits, and the undo gx prints **empties
   > the object with `rc=0`**. The paragraph above stands exactly as written, and a probe now drives
   > that road on every run so that the day it does close, this page moves in the same commit.
   > (`crates/gx-adapter-mcp/tests/r25_declaration_axes.rs`, **seven** tests, beside
   > `crates/gx-cli/tests/r25_abort_and_record_only.rs`'s eight. Windows, OneDrive and network shares
   > are still measured **zero** times, and zero calls were made to github or notion.)

   >
   > v0.5-m (2026-08-19, `req/38` §231, from the twenty-fifth adversarial audit `req/324`):
   >
   > 🔴 **The accepted residual was declared one word wide and the gate admits a family.** The
   > paragraph above says a single `{"const": …}` member satisfies the template gate while carrying
   > nothing of the prior, and v0.5-l's correction — added *in order to record that the residual is
   > spelling-dependent* — named the same single spelling again. The gate's own classifier does not:
   > `ArgSource::draws_from_outside_the_forward_call` writes `Const` and `ConstJson` **on one arm**,
   > and the twenty-fifth audit drove the other spelling on a real binary against the shipped demo
   > server. `{"const_json": ""}` and `{"const_json": null}` reach a terminal state that is not one
   > character different from the named one: accepted, `Admit`, the effect lands, and the `gx undo`
   > gx itself prints empties the object with `rc=0`.
   >
   > **The width, measured rather than described.** Six words draw on something the forward call does
   > not carry, which is what lets a template satisfy this gate on its own: `const`, `const_json`,
   > `prior_contents_utf8`, `prior_json`, `do_result` and `do_result_number_from`. They are **not**
   > one class, and the difference is measured:
   >
   > * `prior_contents_utf8` and `prior_json` draw the **prior**, so the restore call carries what
   >   the object held and puts it back. These are the shapes the gate exists to admit.
   > * `const` and `const_json` resolve at **plan time** from the declaration itself. Both were
   >   driven and both empty the object through the printed undo with `rc=0`. **This is the accepted
   >   residual, and it is two spellings wide, not one.** Spelled the way a catalogue file spells
   >   them, so that a reader searching this page for what they wrote finds it: a single
   >   `{"const": …}` member and a single `{"const_json": …}` member each satisfy this gate while
   >   carrying nothing of the prior.
   > * `do_result` satisfies the gate while carrying nothing of the prior and was measured **not** to
   >   destroy: its printed undo refused with `rc=1` and the object was unchanged. The residual does
   >   not extend to it on this build.
   > * `do_result_number_from` was not driven on this road; it is named here because the classifier
   >   classifies it, and a word this page does not name is the drift this block is about.
   >
   > `const_json` was measured with a string and with `null`. **Array, object and number values were
   > not driven**, and neither was `prior_json` as a template's only outside member. The residual is
   > unchanged in kind and is now stated at the width the code actually has;
   > `crates/gx-adapter-mcp/tests/r26_limits_family_sync.rs` reads the classifier's arms on every run
   > and is red until this page names every word in them, so the day a seventh word is classified,
   > this paragraph moves in the same commit.
   >
   > 🔴 **The invisible-edge axis is now a class, and v0.5-l's "exactly these five" was wrong.** That
   > block wrote *"the width is `char::is_whitespace` plus exactly five scalars: U+200B, U+200C,
   > U+200D, U+2060, U+FEFF"* and *"they are enumerated rather than described"*. The audit walked
   > twelve more scalars that render as nothing at an edge — U+00AD, U+061C, U+115F, U+1160, U+17B4,
   > U+180E, U+200E, U+2061, U+2066, U+3164, U+FFF9, U+E0001 — through the five positions a
   > declaration spells a name in, and **the edge gate stopped none of the sixty cells**. A
   > `$cas_read` prefix padded with any of them parsed and then governed no locator at all, which is
   > the one fault in a catalogue file that produces no error at the moment it matters.
   >
   > The gate now asks a **class**: Unicode's `Default_Ignorable_Code_Point` together with the format
   > category `Cf`, beside `char::is_whitespace`. The union rather than either half, because each
   > half misses part of the axis — the Hangul fillers are letters, and the interlinear annotation
   > anchor is subtracted from Default_Ignorable — and because a property Unicode publishes is a
   > width a reader can check against the standard instead of against a list this crate maintains.
   > Sixty cells of sixty are refused in the gate's own words, and the negative control is the half
   > that matters: real Japanese, Chinese, Korean, Arabic, Hebrew, Cyrillic and Devanagari tool names
   > are driven through all five positions and all thirty-five parse. A gate that refused those would
   > be a worse defect than the one it repairs.
   >
   > **What this still does not close.** The class is about scalars that render as nothing at an
   > **edge**. Inside a name they are all still accepted, exactly as U+200B always was — including a
   > right-to-left override (U+202E), which this build now refuses at either edge and takes in the
   > middle. **Canonical equivalence** (the angstrom sign against A-with-ring) and NFKC confusables
   > are unchanged and still open: they need a normalisation table this build does not carry.
   >
   > 🔴 **A name made only of invisible scalars was declared in five slots and refused in three.**
   > The *unnamed* gates were widened to the same predicate in v0.5-l, and the sweep reached
   > `restored_by`, the read face's `by_tool` and the `$cas_read` face's `by_tool`. It did not reach
   > the two slots that are **keys**: the `restores` key — the effect a declaration is about — and
   > the `$cas_read` prefix. Both took a name that is blank on the page the file was approved on.
   > Eighty-five cells of eighty-five are now the unnamed fault, each in the sentence a reader can
   > act on rather than in the edge gate's.
   >
   > 🔴 **The correction v0.5-l made to its own census was itself one road wide.** That block wrote
   > that the gate *"counts every read of the private map itself"* and argued the sibling files were
   > closed by the language, because the map is a private field. The field is private; the question
   > is not. `spec_for`, `restore_for`, `declared_reversibility` and `writes_per_this_file` are all
   > **`pub`**, and a third gate written `self.spec_for(tool).is_some()` — which asks the key half
   > only, the defect family three releases in a row were each a repair of — moved the census
   > `11 → 11` and the sibling scan `0 → 0`. The census now counts every road to the question, the
   > field and the four accessors alike, in `catalogue.rs` and in every sibling file.
   >
   > 🔴 **The refusal that discriminates two preimages was funnelled through one function and there
   > were two roads.** v0.5-k's sibling sweep published a table of four rows, and all four were
   > consumers of one internal funnel. This crate has a **second** road to the declared read face —
   > the escrow's own `read_by`, in `invert::invert_with_verdict` — which was on no row of that
   > table. The same server decision, a JSON-RPC `-32002`, reached the agent there with no word
   > saying which of the two facts it was. Worse, the sentence was composed as
   > `format!("{REFUSAL} ({error})")`, so **gx's own words pushed gx's own token to offset 509** and
   > the position-dependent predicate the discrimination rests on would have answered `false` even
   > if it had been asked. Both roads now ask the question **before** composing anything, through
   > the one funnel, and the sweep that holds them is derived from the source rather than written
   > by hand.
   >
   > 🔴 **The proxy told an agent why no compensation was attempted, and it could only be right one
   > time in three.** v0.5-l replaced a clause that was false on one of three roll-back values with
   > one arm per value, and then had the *not attempted* arm assert a **cause**: that the escrowed
   > inverse was still partial. The engine constructs that value at three places with three
   > different causes — a partial escrow, an escrow that was never built at all, and a `gx repair`
   > recovery path that rebuilt nothing. The cause now travels with the value, the proxy has one arm
   > for each and one for a cause it does not recognise, and on an unrecognised cause it says what
   > the value carries and declines to put words in the engine's mouth.
   >
   > **What has still not been driven on a real road**, said here rather than left to be found: the
   > two roads that reach *not attempted* other than the partial escrow. The approval path that
   > reaches an escrow answering `None`, and `gx repair`'s recovery path, are structure this release
   > repaired and roads the twenty-sixth audit is charged with driving.
   >
   > (`crates/gx-adapter-mcp/tests/r26_invisible_edge_axis.rs`, **eight** tests, beside
   > `crates/gx-adapter-mcp/tests/r26_preimage_funnel.rs`'s six,
   > `crates/gx-adapter-mcp/tests/r26_reach_census.rs`'s four,
   > `crates/gx-adapter-mcp/tests/r26_limits_family_sync.rs`'s three,
   > `crates/gx-cli/tests/r26_not_attempted_causes.rs`'s nine and
   > `crates/gx-cli/tests/r26_refusal_remedy_parity.rs`'s four. Windows, OneDrive and network shares
   > are still measured **zero** times, and zero calls were made to github or notion.)
   >
   > v0.5-n (2026-08-19, `req/38` §233, from the twenty-sixth adversarial audit `req/329`):
   >
   > 🔴 **The class stopped short of two categories, and the list of what it does not close did not
   > mention them.** The block above widened the invisible-edge axis from five enumerated scalars to
   > `Default_Ignorable_Code_Point ∪ Cf`, and argued — rightly — that a property Unicode publishes is
   > a width a reader can check against the standard. The twenty-sixth audit walked what that
   > argument leaves out. Control scalars at an edge: **25 of 30 cells parsed**. Private-use scalars
   > at an edge: **10 of 10 parsed**. The five that stopped were U+0085, and they stopped because
   > `char::is_whitespace` answers for it — not because the class reached it. A `$cas_read` prefix
   > padded with U+0007 parsed and then governed no locator at all, which the paragraph above calls
   > the one fault in a catalogue file that produces no error at the moment it matters.
   >
   > What makes this a defect and not an omission is that the block above **enumerates** what the
   > class still does not close — inside-a-name, canonical equivalence, NFKC confusables — and these
   > were not on that list. A document that enumerates and then omits is read differently from one
   > that stays silent.
   >
   > The gate now also asks `General_Category=Cc` and `General_Category=Co`. Both are categories
   > Unicode publishes, so the width remains checkable against the standard rather than against a
   > list this crate maintains, and the two halves are claimed differently on purpose: `Cc` renders
   > as nothing or as a control picture, which is the axis the gate names for itself, while a
   > private-use scalar **renders as whatever a private agreement says** — not invisible, but not a
   > fact the page an operator approves carries either. The question a catalogue asks is whether the
   > person approving the file can read the name it declares, and a name whose glyph is outside the
   > standard cannot be read off that page at all. The private-use half is stated as that and not as
   > a claim of invisibility. The two supplementary planes are included with the BMP area; the range
   > table's `E0000-E0FFF` was plane 14's tag range, which is a different thing with a similar
   > spelling.
   >
   > The negative control is driven before the widening and is what licenses it: seven real tool
   > names in Japanese, Chinese, Korean, Arabic, Hebrew, Cyrillic and Devanagari through the five
   > positions, and **all thirty-five parse** after the class grew. A boundary arm drives Latin,
   > Hiragana and an emoji through the same positions and requires the edge gate to answer for none
   > of them, so a later lane that widens past the two categories sees the collision.
   >
   > **What this still does not close.** Everything the block above left open stays open and is not
   > restated here except to say it is unchanged: inside a name every one of these scalars is still
   > accepted, canonical equivalence and NFKC confusables still need a normalisation table this build
   > does not carry. Two further limits are named here for the first time. **Unassigned** scalars are
   > outside the class deliberately — refusing every scalar a build has not heard of would rot the
   > day Unicode assigns one — and the **downstream** consequence of a control-spelled name is
   > measured **zero** times: what this release closes is that such a name is refused at parse, and
   > what a `restored_by` spelled with U+0007 would have done at undo time was never driven, by the
   > audit or by this repair.
   >
   > 🔴 **A numeric claim stopped being checked the moment the anchor moved past it.** `limits_sync.rs`
   > holds *the newest stacked block* and nothing else, so when the previous release moved the anchor
   > forward, every number the outgoing block had stated left the checked set in the same commit. One
   > of them was false by then, and false because of that same release: an arm was added to
   > `crates/gx-cli/tests/r25_abort_and_record_only.rs`, `limits_sync`'s own declaration was raised
   > from eight to nine, and the sentence on the page went on saying **eight**.
   >
   > Asking the same question of the whole page turned up two more claims, both older, that no later
   > block corrects. Neither is a claim about this build's behaviour — they are counts of probes — but
   > a page that is wrong about how much was measured is wrong about the weight of everything it says.
   >
   > **Current, as of this block.** The older sentences stand where they are, because this page is
   > additive and rewriting a block would destroy the record of what was believed when it was written:
   >
   > * `crates/gx-cli/tests/r25_abort_and_record_only.rs` has **nine** tests; the v0.5-l block says
   >   eight.
   > * `crates/gx-cli/tests/serve_runtime_r3.rs` has **eight** tests; the block that introduced it
   >   says seven.
   > * `crates/gx-cli/tests/serve_runtime_r6.rs` has **sixteen** tests; the block that introduced it
   >   says fifteen.
   >
   > The convention that makes this the last time: an anchor move is a decision, and for every
   > numeric claim in the outgoing block a lane either carries it into a registry that outlives the
   > anchor or writes the correction above. `crates/gx-cli/tests/r27_limits_probe_counts.rs` holds the
   > registry and is red until the page carries a current statement for every suite in it, and
   > `limits_sync.rs` records the rule beside the list of anchor moves where the next lane will read
   > it. `probes/doubt/tests/declaration_writer_doubt.rs` is what the convention looks like when it
   > works — `probes/doubt/tests/declaration_writer_doubt.rs` has **seven** tests, and an early
   > block says five, a later one says *"now seven"*, a later one *"still seven"* —
   > and it is why the rule is that the page's **last** statement must be true rather than that every
   > statement must be.
   >
   > 🔴 **What the residual accepts does not depend on the value's type, and one member of the family
   > does not destroy.** The block above measured the accepted template residual with a string and
   > with `null`, and said in as many words that array, object and number values were not driven, and
   > neither was `prior_json` as a template's only outside member. The twenty-sixth audit drove all
   > five on a real binary against the shipped demo server. Array, object, number and boolean reach a
   > terminal state **not one character different** from the string spelling already reported:
   > accepted, `Admit`, the effect lands, and the `gx undo` gx itself prints empties the object with
   > `rc=0`. So the residual is two spellings wide as stated, and
   > **the value's type is not one of its dimensions** — a narrower and more useful thing to say
   > than the block above could say.
   >
   > `prior_json` as a template's only outside member is the opposite result and is recorded as such:
   > the gate passes, but the prior is not JSON on that bed, so no inverse can be built, E-M3-4
   > escalates, nothing is sent and the object does not move (`rc=6`, the object still holding what it
   > held). It joins `do_result` on the short list of members **measured** not to destroy, rather than
   > assumed not to.
   >
   > (`crates/gx-cli/tests/r27_reentrant_abort.rs` has **six** tests,
   > `crates/gx-adapter-mcp/tests/r27_edge_class_width.rs` has **six**,
   > `crates/gx-adapter-mcp/tests/r27_census_derivation.rs` has **six**,
   > `crates/gx-cli/tests/r27_parity_allowlist.rs` has **six** and
   > `crates/gx-cli/tests/r27_limits_probe_counts.rs` has **five**. Windows, OneDrive and network
   > shares are still measured **zero** times, no calls were made to github or notion, and the
   > downstream consequence of a control-spelled name at undo time is still **zero** runs.)
   >
   > v0.5-o (2026-08-19, `req/38` §235, from the twenty-seventh adversarial audit `req/334`):
   >
   > 🔴 **An undo that stopped halfway would not say what became of the roll-back, on the
   > only face a GUI speaks.** The twenty-seventh audit drove `POST /transformations/{id}/undo`
   > against a substrate that refuses the inverse's apply and read the answer: keyed on the abort
   > taxonomy, and carrying the word `rollback` zero times. It then drove the variant where the
   > roll-back *itself* also fails and got a body identical in every member. Two terminal states --
   > the object is back where it was, and the object is half undone -- were one answer. The value
   > was in the engine the whole time, and this build's own command line had been printing it since
   > M6. Both facts are now **members** of the problem object (`rollback` and
   > `rollback_not_attempted_because`), on the abort roads and on no others, so their presence is
   > itself the signal; RFC 9457 §3.2 provides for the extension and `retry_after_ms` is the
   > precedent this follows. `null` is written rather than omitted, because an absent member cannot
   > be told from a road that never had the fact.
   >
   > The census that should have caught it is replaced rather than widened. R27's sweep walked one
   > crate and selected on one surface's JSON member name, and **a census whose selector is one
   > surface's wire vocabulary cannot see another surface however wide its directory is made.** The
   > new sweep derives its denominator twice from the source -- which abort reasons the engine ever
   > records a roll-back for, and which crate declares the accessor -- and walks every crate. It
   > finds five roads across two crates today, and a sixth added tomorrow enters on the day it is
   > written.
   >
   > 🔴 **The reachability census could not see two spellings of the road it counts.** It
   > tested `&self` against a signature's first physical line, so a signature rustfmt wrapped
   > stopped being a road -- and `catalogue.rs` already writes nine signatures in that shape. A
   > method behind a trait carries no `pub` and was invisible for the same kind of reason. Both are
   > counted now, against the whole signature and through trait impls, and a builder is still not a
   > road.
   >
   > 🔴 **The late-escrow completion road folded four facts into one word and threw the
   > sentence away.** Four different failures answered `Ok(None)` and the engine records
   > `InverseStatus::Unavailable` for all four -- a word 42 §3.12 defines as *`invert()` returned
   > `None`*. One of the four is not that: the inverse **was** derivable and **was** escrowed, and
   > what failed is that the applied call's observation did not carry a member the declaration
   > named. An operator told `Unavailable` gives up; an operator told the truth fixes the read face
   > and the undo becomes possible. Worse, in that case a sentence naming the pointer and the
   > reason had already been composed and was being dropped on the floor. Each arm now names its
   > fact and the sentence reaches the call log. **The fold at the engine is unchanged and is
   > declared here rather than repaired**: `Ok(None)` is a deliberate fail-safe, and minting a
   > seventh `InverseStatus` word is a wire ruling and not a repair lane's. So: **a `Pending`
   > escrow that could not be completed still reaches a reader as `Unavailable`, and the reason for
   > it is in the adapter's log rather than on the wire.**
   >
   > 🔴 **The page carried a number that was false, the registry built to stop that agreed
   > with it, and the control built to check the registry agreed too.** All three stood on one way
   > of counting -- the characters `#[test]` anywhere in a file, including inside a string literal
   > -- and exactly one registered file quotes that literal: the registry's own, in its counter. So
   > the counter counted itself. `req/332` §10-2 records this row being *corrected* from four to
   > five; the correction moved the number away from the truth because it trusted the counter.
   > **Three layers agreeing is not three layers when all three stand on one computation.**
   > Counting is now by attribute at the start of a line, cross-checked against the items those
   > attributes introduce -- two methods that fail differently -- and the control that proves they
   > can be made to disagree lives in its own file, because an arm added to the registry would
   > change the very number it measures. Correcting the block above:
   > `crates/gx-cli/tests/r27_limits_probe_counts.rs` has **four**, not the five it states.
   >
   > 🔴 **Cell counts are now held by numbers rather than by strings, and one was already
   > stale.** The blocks above state cell counts as well as probe counts, and the registry's
   > selector wants a `/tests/` path, so a claim with no path in it was structurally outside. The
   > arm pinning the widest of those claims asked only that the string `all thirty-five parse` be
   > present -- so a tree driving forty cells and a page saying thirty-five would both stay green.
   > Wiring the page's numerals to the arrays the arms actually drive found that the private-use
   > claim above (**10 of 10**) had been left behind by the same block that widened it: two rows
   > became four when the supplementary planes were added, and nothing said so. This tree drives
   > **thirty-five cells for the negative control**, **thirty cells for the control scalars** and
   > **twenty cells for the private-use areas**, derived from the declarations on every run.
   >
   > 🔴 **Refusals had two spellings for the marker that introduces their remedy** --
   > `What to fix:` and the retired one -- and the parity gate knew one of them and scanned one
   > directory, so widening the scan would have filed real remedies as missing ones. Read end to
   > end, the distinction is not a rule anyone was following: the same receipt-archive failure
   > carried one marker in `gx-engine` and the other in `gx-api`. The vocabulary is now one
   > spelling across every crate, the sentences keep their own words, and the census that says so
   > rejoins backslash-continuations before it counts -- without which it misses one occurrence of
   > each spelling, as the naive count in the audit that filed this did.
   >
   > (`crates/gx-cli/tests/r28_abort_answer_sweep.rs` has **four** tests,
   > `crates/gx-cli/tests/r28_probe_counter_discrimination.rs` has **three**,
   > `crates/gx-cli/tests/r28_remedy_marker.rs` has **three**,
   > `crates/gx-api/tests/r28_rollback_members.rs` has **three**,
   > `crates/gx-adapter-mcp/tests/r28_completion_facts.rs` has **four** and
   > `crates/gx-adapter-mcp/tests/r28_cell_count_claims.rs` has **three**. Windows, OneDrive and
   > network shares are still measured **zero** times, no calls were made to github or notion, the
   > downstream consequence of a control-spelled name at undo time is still **zero** runs, and the
   > `gx serve` runtime -- socket bind, graceful shutdown, `GET /stream` -- is still **zero** runs:
   > every HTTP measurement behind this block was driven in-process through the shipped router.)
   >
   > v0.5-p (2026-08-20, `req/38` §238, from the twenty-eighth adversarial audit `req/361`):
   >
   > 🔴 **`Succeeded` was a word about a call, and it was printed over a broken world.** When a
   > commit's apply fails, 43 T-10c sends the escrowed inverse back to the substrate, and until this
   > window the engine wrote `rollback: "Succeeded"` the moment the adapter answered `Ok` -- without
   > reading the object. The twenty-eighth audit drove a **contract-conforming** adapter whose
   > `apply` can fail halfway (`gx-substrate`'s own `ApplyFailed` doc says "a non-atomic `apply` can
   > fail halfway"), and the bytes on disk went `A B C D` -> `A B D` (the forward apply stopped
   > mid-way) -> `A B D C D` (the inverse then ran **completely and honestly**). The delivered body
   > said `"rollback":"Succeeded"` over that, while the same body's prose claimed to be "the
   > difference between an object that is back where it was and one that is half undone". It was
   > not. **The roll-back now reads the object back and compares it with the fingerprint the
   > transformation started from**, and a world that is not home gets its own word, `Diverged`.
   >
   > 🔴 **What that read is not.** It is one read taken after the call, mirroring the second forward
   > CAS R8 added -- and it inherits R8's honesty. **It is not atomicity**: between the inverse's
   > apply returning and this read there is a window, and a third party who writes inside it can
   > make a homecoming look like a divergence or the reverse. **It is not attribution**: `Diverged`
   > says *the object is not at `fp0`*, never *the roll-back moved it*; one fingerprint cannot tell
   > "the compensation overshot" from "somebody else wrote". **It is not a distance**: a fingerprint
   > is an equality, so an object one byte from home and an object unrecognisable get the same word.
   > And a read that cannot be taken at all is reported as `Failed` rather than as a fifth word,
   > because `Failed`'s sentence already covered "a compensation whose bytes landed and whose
   > read-back died". What is removed is exactly one thing: **the terminal state a reader is handed
   > can no longer be `Succeeded` over an object that is demonstrably not where it started.**
   >
   > 🔴 **Which adapters this is about.** The audit's adapter was written for the audit. **Whether
   > the shipped `gx-adapter-git` / `-mcp` / `-postgres` deltas are relative or absolute -- that is,
   > whether any shipped adapter can actually reach this state -- is measured zero times.** The
   > v0.1 fs delta cannot: M4-13 (a) fixed it to the one shape where `rename` makes a half-apply
   > impossible. So the repair is stated as closing a hole the **contract** permits, not one a
   > shipped adapter has been observed to fall into.
   >
   > 🔴 **The roll-back fact now reaches the three read faces, not only the refusal.** Until this
   > window a client speaking only HTTP could read `state: {"Aborted":"ApplyFailed"}` -- the abort
   > reason was answerable -- and had no road at all to *is my object back where it was*, unless it
   > happened to be holding the refusal from the request that aborted. `GET /transformations/{id}`,
   > `GET /transformations` and the `aborted` event on `GET /stream` now each carry an optional
   > `rollback` member (44 §2.6's backward-compatible addition; `null` where no roll-back was in
   > question). All three at once, because a ruling taken on two of them leaves the third behind.
   >
   > 🔴 **Correction to v0.5-o's marker sentence, above (the old text stands).** That paragraph says
   > the rejoining census "misses one occurrence of each spelling". **It misses three: two of the
   > kept spelling and one of the retired one.** Re-derived twice -- by the twenty-eighth audit and
   > independently by this window, re-implementing the gate's own line-oriented rejoin over the base
   > it was written against (`f1fbd9d`, `crates/*/src`, 138 files): naive `25 / 9`, rejoined
   > **`27 / 10`**, split across a continuation in `gx-adapter-mcp/src/catalogue.rs`,
   > `gx-adapter-mcp/src/invert.rs` (named by neither sentence) and `gx-cli/src/repair.rs`. The
   > gate's own verdict only balances on the corrected number: `27 + 10 = 37` at that base, plus the
   > three sentences R28 added, is the **40** the gate prints today. The measurement and the gate
   > were right throughout; what drifted was this page and the gate's own doc comment.
   >
   > 🔴 **Precision, one word, on the `CallLog` sentence above.** v0.5-o says the reason for an
   > uncompletable `Pending` escrow "is in the adapter's log rather than on the wire". Read strictly
   > that is a promise about a `pub` trait anyone downstream can implement, and `CallLog::note`'s
   > default implementation **discards**. On every road this build ships it holds -- the one
   > non-test construction site takes the default `MemoryCallLog`, which keeps what it is told -- so
   > the sentence is true of the shipped binary and was never true unconditionally. It is now
   > written as **the log a deployment wired**.
   >
   > 🔴 **Three census walks were correct only by the spelling habits of the directory they read.**
   > Three live gates accumulated a constant's body until a line ended in the two characters `";`,
   > and selected constants by `contains("&str")` -- which does not match `&'static str`. Measured:
   > **today nothing is swallowed** (27 constants selected, 27 reaching a terminator), and one
   > ordinary alternative spelling spliced in made the walk run past the end of the declaration
   > silently. Both are closed, and the walk now **panics by name** rather than returning a
   > truncated body.
   >
   > 🔴 **Correction to the v0.5-m block's probe count, above.** That block says
   > that suite's **four**; this window added the arm that drives the repaired walk rather than
   > reading it, so `crates/gx-cli/tests/r26_refusal_remedy_parity.rs` has **five** and that
   > block's crate total is **35**, not 34. The page is additive and the older sentence stands as the record of
   > what was true when it was written; this is the reader's current answer.
   >
   > (`crates/gx-cli/tests/r29_rollback_is_verified.rs` has **six** tests,
   > `crates/gx-cli/tests/r29_instrument_repairs.rs` has **five** and
   > `crates/gx-api/tests/r29_rollback_read_faces.rs` has **four**. Windows, OneDrive and network
   > shares are still measured **zero** times, no calls were made to github or notion, the shipped
   > non-fs adapters' delta grammars are measured **zero** times, and the `gx serve` runtime is
   > still **zero** runs in this window: every HTTP measurement behind this block was driven
   > in-process through the shipped router.)
   >
   > v0.5-q (2026-08-20, `req/38` §240, from the twenty-ninth adversarial audit `req/372`):
   >
   > 🔴 **The automatic roll-back used to overwrite whatever it found, and one of the things it
   > found was other people's work.** When a commit's apply fails, 43 T-10c sends the escrowed
   > inverse back to the substrate. Until this window it sent it **unconditionally**. Every shipped
   > adapter's delta grammar is absolute — the twenty-ninth audit drove all four to establish it —
   > so the inverse restores from *any* world, including one somebody else legitimately created.
   > Measured, on a real branch, with the shipped git adapter:
   > `prior=de05de3`, a colleague commits `d2d09b5`, the compensation runs, `after_rollback=de05de3`,
   > `their_commit_is_still_the_tip=false`, and the word written over it was **`Succeeded`**. The
   > word was not lying: `fp0` is a statement about *this* transformation's object, and it says
   > nothing about whose work was standing on it. The identical failure was already measured and
   > repaired one road over — v0.4-o, above, says of `gx undo` that "the escrowed inverse was
   > written over the top and the other change was gone with no message" — and the repair had gone
   > into the road **a person** starts and never reached the road **nobody** starts, which is also
   > the road with no operator standing over it to stop it.
   >
   > 🔴 **The engine now reads the object twice before it compensates, and the second read is not
   > compared with `fp0`.** This is the part worth reading carefully, because the obvious repair is
   > wrong and we shipped it into a test suite before it was caught: *somebody else wrote* and *our
   > own apply landed* are the **same observation** when the only thing compared is `fp0` — in both
   > the object is simply not where the transformation started. A guard on `fp0` alone therefore has
   > to sacrifice one of them, and the first draft sacrificed the wrong one: the ordinary case this
   > entire road exists for, a call that **landed and then errored**, silently stopped being
   > compensated. The information the two cases differ by is **time**, not fingerprints. So:
   >
   > | what the reads find | what happens |
   > |---|---|
   > | the object is at `fp0` when the apply fails | the apply moved nothing; **no inverse is sent** (`NotAttempted`, `WorldNeverMoved`) |
   > | it moved, and is still where the apply left it | the compensation runs, exactly as before |
   > | it moved, and then moved **again** | somebody wrote in between; **no inverse is sent** (`NotAttempted`, `WorldMovedBeneath`) |
   > | either read will not answer | **no inverse is sent** (`NotAttempted`, `WorldCouldNotBeRead`) |
   >
   > The first row is the audit's worst shape closed at its root — *a transformation that did
   > nothing erasing a third party's write and nothing else* — and nothing is given up by declining
   > there, because an absolute inverse over a world at `fp0` is a no-op and a **relative** one is
   > worse than a no-op.
   >
   > 🔴 **What is still true after it, said as a width rather than as an adjective.** The compare and
   > the write are two calls, so a third party who writes between them is still overwritten, and so
   > is one who writes *during* the inverse's own apply. Measured on this build, n=101, and **run
   > twice, with both runs reported rather than the better one**: the window
   > `[precondition returns, apply entered]` has a median of **0.189 / 0.187 µs** on fs (tmpfs) and
   > **0.153 / 0.128 µs** on mcp, against an instrument floor (back-to-back clock reads) of
   > **0.018 µs**, so it is resolved rather than quantisation. The inverse's own apply — the rest of
   > the exposed bracket, since a writer who lands *during* it is overwritten too — has a median of
   > **35.2 / 37.3 µs** on fs and **15.2 / 15.2 µs** on mcp. 🔴 **These are lower bounds.** The
   > comparable figure the twenty-ninth audit published, `A29_ROLLBACK_WINDOW median_us=22.6`, is
   > **not** term-for-term the same bracket — it timed `[apply returns, read returns]`, because
   > before this window the only read was the one *after* the write — and neither number should be
   > quoted as the other. What the figures exclude is
   > declared rather than smoothed over: the engine appends an `ApplyStarted` journal record inside
   > the window and that append is **not** in these figures; the mcp fixture is **in-process** —
   > same thread, a map behind a mutex, crossing no socket and framing no JSON-RPC — so a real MCP
   > wire is measured **zero times** and the mcp figure is a floor under a floor; and cold caches,
   > slower substrates and `fsync`-honouring filesystems are measured zero times each. It is also
   > **not attribution**: a second writer of your own, another agent and a CI job are one
   > observation here, and so is this call's own apply if it moved the object twice.
   >
   > 🔴 **Correction to v0.5-p's reachability sentence, above (the old text stands).** That block
   > wrote *"whether the shipped deltas are relative or absolute — **that is**, whether any shipped
   > adapter can actually reach this state — is measured zero times."* The `that is` equates two
   > different questions and the twenty-ninth audit answered them separately: the shipped grammars
   > are **all absolute** (all four, `MAX_OPS = 1` enforced at decode, so a half-apply cannot be
   > written as a payload), **and `Diverged` is still reachable** — measured, with the shipped
   > `gx-adapter-mcp`, **without a half-apply anywhere in it**
   > (`A29_DIVERGED_REACHABLE word=Diverged half_apply_involved=false`). Half-application is **one**
   > road to that word, not the definition of it. There are three, and after this window they are:
   > (1) a deployment-declared restore template that does not name the prior state (already declared
   > at v0.5-m); (2) the residual window above; (3) a delta grammar that is not absolute, which no
   > shipped adapter has. Reading the corrected sentence as *"absolute, therefore `Diverged` cannot
   > happen"* is the misreading the old wording invited, and it is refuted by measurement.
   >
   > 🔴 **Correction to v0.5-p's subject, above (the old text stands).** That block says "**The
   > roll-back now reads the object back**". The reader is the **adapter**, not gx: the sequence is
   > `adapter.snapshot` → `adapter.precondition` → `fp0.cas_eq`. The four shipped adapters all read
   > honestly, so there is no difference today — but where a deployment's `$cas_read` declaration
   > names a different object (which v0.5-h already declares gx will not notice), `Succeeded`,
   > `Diverged` and every word in the table above are words about **the adapter's view**, not about
   > the object. The same correction applies to the two new reads this window added.
   >
   > 🔴 **And a correction to what `req/361` §9-2 said was missing on the forward side.** R29
   > recorded that the forward apply had no read-back and that the only reason was "not done yet".
   > That was wrong, and the twenty-ninth audit re-derived it from the four adapters: every one of
   > them already reads the world after a forward apply (`observe` in fs/git/mcp, a second `SELECT`
   > in postgres). The asymmetry was never a missing read — it was that the forward side's
   > `AppliedDelta` **carries** its observation while the roll-back's `apply` returns `Ok(())`,
   > which says nothing about the world. R29 closed exactly that, and a further `precondition` call
   > on the forward side would buy nothing but one more window: a lying adapter is not caught by
   > asking the same adapter twice. **No forward-side work is outstanding from that entry.**
   >
   > 🔴 **The journal's record vocabulary is versioned now, and the binaries already in people's
   > hands are not reached by it.** The framing was versioned from the start; the vocabulary inside
   > it was not, and R29 added a word (`Diverged`) to a value that is serialised into the `Aborted`
   > record. The twenty-ninth audit stopped reasoning about that and measured it, by building the
   > journal and pointing a **pre-R29 binary** at it: the older binary did not refuse the file. It
   > returned `Ok`, read **one** record of three, reported the remaining 270 bytes as a **torn
   > tail** — the ordinary shape of a crash — quarantined them, cut the live file from **415 bytes
   > to 145**, and after one ordinary append the journal looked *healthy*, with two records in it
   > and none of the lost history. The sentence the operator was shown named `gx repair` as the
   > remedy, and `gx repair`'s own documentation says what it cannot do is put them back. This was
   > never data destruction — the bytes are quarantined — it was **a confident diagnosis of the
   > wrong illness and a treatment that does not touch the right one**.
   > From this window, a journal this build creates carries the marker `GXJRNL02` with its own
   > genesis link; a record whose vocabulary is newer than its journal's framing is **refused rather
   > than written**; a file whose marker this build does not know is refused **whole** — not
   > truncated, not quarantined — and `gx` says "the bytes decode as records of a newer vocabulary"
   > instead of describing a crash.
   > **What this does not do, in the plainest words available: it does not reach any binary released
   > before this one.** That is not an expectation — the same pre-R29 binary (`3c2cf32`) was built
   > again and pointed at a journal this build wrote, and **the two roads it can be called on give
   > two different answers, so both are published:**
   >
   > | road | what the old binary does with a `GXJRNL02` journal |
   > |---|---|
   > | **declared** (`.gx/VERSION` names the format — every project `gx init` makes) | 🔴 **refuses.** `file_bytes 415 -> 415`, nothing quarantined, and the append is refused in words. R6's downgrade guard fires, because a marker it cannot read makes the file sniff as `legacy` against a `chained` declaration |
   > | **undeclared** (an embedder calling `EngineJournal::open`) | 🔴 **still truncates, and truncates more than before.** `records=0 torn_tail_bytes=415`, the file cut `415 -> 0`, the next append accepted, and the journal ends at 100 bytes reporting `records=1 torn=0` — healthy-looking, with **315 bytes** gone where the pre-R30 journal lost **138** |
   >
   > The second row is a **regression on that road and is not rounded up into the first**: the new
   > marker moves the misreading earlier (byte 0 instead of byte 145) rather than converting it into
   > a refusal, because the only thing that produces a refusal on an old binary is a declaration it
   > can compare the file against, and the undeclared road has none. There is no byte sequence this
   > build could write that would make an already-released binary refuse without one. **What is
   > bought is the next version window and the ones after it** — this build refuses a framing it
   > does not know, whole and by name — **and the cost of that purchase is stated in the next
   > paragraph rather than left for an upgrader to discover.**
   > 🔴 **The cost.** A project created **before** this release keeps its `chained` framing, because
   > a chain cannot be re-framed in place (the genesis link is minted over the marker, so changing
   > it invalidates every link after it). On such a project, an outcome that needs a v2 word cannot
   > be journalled: the record is refused and the verb fails instead of recording an abort. The
   > exposure is small for a measured reason rather than a hopeful one — the same window's other
   > half removed the roads on which `Diverged` was easiest to reach, so a shipped adapter now gets
   > there only through the residual window measured above.
   >
   > v0.5-r (2026-08-20, `req/38` §242, from the thirtieth adversarial audit `req/378`):
   >
   > 🔴 **Correction to v0.5-q's residual-window figures, above (the old text stands).** The
   > numbers v0.5-q published for `[precondition returns, apply entered]` — **0.189 / 0.187 µs** on
   > fs — and for the inverse's own apply — **35.2 / 37.3 µs** on fs — are **withdrawn as
   > measurements of this system**, and the reason is worth more than the numbers. They were not
   > taken by driving the engine. `crates/gx-adapter-fs/tests/r30_rollback_window.rs` says so in
   > its own words — *"this is a **reconstruction**, not the engine's own instrumented timing"* —
   > and `gx-adapter-fs` cannot do otherwise, because `gx-engine` is not among its
   > dev-dependencies. That suite is registered rather than left behind —
   > `crates/gx-adapter-fs/tests/r30_rollback_window.rs` has **one** probe — so the withdrawn
   > numbers stay inside the checked set instead of drifting out of it the next time this
   > anchor moves. A reconstruction assembled from the adapter's calls **cannot contain the
   > engine's own work between them**, and the engine's own work is what dominates this bracket.
   > `req/38` §241 ruling 3 had already put "no engine-free hand-reconstruction" on the *audit's*
   > instruments; it had not been applied to the figures the product publishes, and this is that
   > application.
   >
   > 🔴 **The same brackets, driven through the engine.** n=101, each run twice, both runs
   > published rather than the better one, on one machine (WSL2, ext4 on an NVMe-backed vhdx) with
   > the substrate swapped underneath the bed to isolate the filesystem. The thirtieth audit's
   > figures are given beside this lane's independent re-measurement, because two machines
   > disagreeing by a fifth is information a buyer should have:
   >
   > | window | tmpfs (audit 30 / R31) | ext4, `fsync` honoured (audit 30 / R31) |
   > |---|---|---|
   > | W1 apply fails → read 1 | 1.550 / 1.579 — 1.539 / 1.587 µs | 6.077 / 5.799 — 5.441 / 5.215 µs |
   > | W2 read 1 → read 2 | 0.623 / 0.632 — 0.608 / 0.609 µs | 1.923 / 1.751 — 1.563 / 1.458 µs |
   > | **W3 read 2 → compensating apply** | **10.873 / 10.854 — 10.627 / 10.731 µs** | **6,869.651 / 6,698.310 — 6,844.632 / 6,506.380 µs** |
   > | W4 compensation → read-back | 0.315 / 0.316 — 0.352 / 0.362 µs | 1.148 / 0.986 — 0.972 / 0.925 µs |
   >
   > **W3 is the exposed residual**, and on a filesystem that honours `fsync` it is **about
   > 6.6 milliseconds** — not the 35 microseconds v0.5-q named for the comparable stretch, and not
   > the 0.189 µs it named for the compare-to-write gap. Against the withdrawn 0.189 µs figure that
   > is roughly **57×** on tmpfs and roughly **35,000×** on ext4; between the two filesystems, on
   > identical code, it is **about 610× (audit 30) / 640× (R31)**.
   >
   > 🔴 **The term that was excluded is the term that dominates.** v0.5-q declared the exclusion in
   > bold — *"the engine appends an `ApplyStarted` journal record inside the window and that append
   > is **not** in these figures"* — so no sentence there was false. What was missing was the
   > multiplication. The call sequence shows nothing between `precondition_in#5` and `apply_in#2`, so the
   > whole 6.6 ms is engine-side, and it is `journal_append` → `barrier()` → `file.sync_all()`: a
   > real durability barrier, doing exactly what it exists to do. **An honest exclusion that
   > changes the answer by four orders of magnitude is still a number a buyer was not given**, and
   > that is the standard this line now holds itself to.
   >
   > 🔴 **A real MCP wire, measured for the first time.** v0.5-q said a real wire was measured
   > **zero times** and called the mcp figure "a floor under a floor". That was the right shape and
   > the floor is now filled in: the shipped `StdioClient`, with `mcp_probe_server` spawned as a
   > **child process**, crossing pipes and framing JSON-RPC, gives a `tools/call` round trip with a
   > median of **207.7 µs** (audit 30) and **176.5 / 192.6 µs** (R31, two runs) against the
   > published in-process figure of **15.2 µs** — **13.7× / 11.6×**. The server's own arrival count
   > (203 = 1 + 2×101) matches the client's, so these are not numbers the client answered locally.
   > This is still a warm local pipe and therefore still not an upper bound: a wire that crosses a
   > network is slower again, and that remains measured **zero times**.
   >
   > 🔴 **Correction to v0.5-q's undeclared-road sentence, above (the old text stands).** That
   > block wrote: *"the only thing that produces a refusal on an old binary is a declaration it can
   > compare the file against, and the undeclared road has none. **There is no byte sequence this
   > build could write that would make an already-released binary refuse without one.**"* The
   > reason clause is **false**, and the sentence built on it is withdrawn. It was not refuted by
   > argument: `3c2cf32` — the last release before this vocabulary existed — was built again and
   > pointed at two files of **45 bytes each, differing only in their first eight**, with a link
   > that is not the link their contents produce. No declaration was involved on either road;
   > `EngineJournal::open` takes a path and nothing else.
   >
   > | first eight bytes | what the pre-R29 binary does | the file afterwards |
   > |---|---|---|
   > | `GXJRNL01` + a link that does not verify | **refuses in words**, naming DR-43-9, and appends nothing | 45 → **45** (untouched) |
   > | `GXJRNL02` — the marker this build actually stamps | reads it as an unknown framing, calls all 45 bytes a torn tail, **truncates**, and accepts the next append | 45 → **0** |
   >
   > A control run in the same suite establishes that the refusal is a property of the bytes and
   > not of the bed: a well-formed journal written by that same old binary accepts a second append
   > normally.
   >
   > 🔴 **What that means, said plainly.** An already-released binary has **two** pre-decode
   > channels, not one, and their postures are opposite. The **marker** channel is **fail-open**:
   > a marker it does not recognise makes the file's whole contents look like debris, and it
   > removes them. The **link** channel is **fail-closed**: a chain link that does not verify makes
   > it stop and leave every byte where it lies. That channel has been wired into every binary
   > shipped since R5. R30 put the new vocabulary's version on the **fail-open** one. So the 315
   > bytes the undeclared road loses are **the cost of the channel that was chosen**, not a law of
   > the format — and v0.5-q presented them as the latter. Publishing an unfalsifiable sentence in
   > a document that sells falsifiability is the part of this worth apologising for; the missing
   > bytes are the smaller half.
   >
   > 🔴 **And why the obvious repair is not being shipped in the same breath.** Framing a v2 journal
   > as `GXJRNL01` while minting its chain from the v2 genesis would make an old binary refuse
   > non-destructively on **both** roads, and the genesis links already differ, so half of it
   > exists. It is not shipped because it buys that protection only for a journal that **holds at
   > least one record** — the link channel needs a record to disagree about — while making an
   > **empty** v2 journal indistinguishable from an empty v1 one by its bytes alone. This build
   > would then have to learn its own framing from the declaration again, which is the second
   > source of truth the thirtieth audit's other finding (H-02) was raised to remove and this
   > release removes. Trading a repaired fault for a reopened one is not an improvement, so the
   > design is recorded, the trade is stated, and the choice is left where a format decision
   > belongs rather than being made inside a repair. **What is fixed today is the sentence.**
   >
   > v0.5-s (2026-08-21, `req/38` §249, from the thirty-second adversarial audit `req/397`):
   >
   > 🔴 **Correction to v0.4-s above (the old text stands). "It refuses without writing anything"
   > was not true, and gx said it in the refusal itself.**
   >
   > v0.4-s, in the paragraph about a recovery run under the wrong signing key, told you that where
   > there is no receipt yet gx "**refuses without writing anything**, names the key you need, and
   > the correct key still finishes the job afterwards". The last two clauses were true. The first
   > was not, and the sentence gx printed on the way out — "**Nothing was applied** and no terminal
   > record was written" — was not either.
   >
   > What the audit measured, on this build and with no tampering at all: a project interrupted
   > inside 43 §7-3b's window and recovered under a second key called `adapter.apply` **once**
   > before it compared anything, and only then refused. On a filesystem the second write is the
   > same bytes, so nothing looks different; on a substrate reached through a tool call it is a
   > second call. The audit then pointed a journal's open row at a delta the project already held —
   > a journal whose links are recomputed end to end verifies perfectly, which this page has said
   > since v0.4-r — and watched an operator's file go from `two` back to `one` while the same
   > sentence said nothing was applied and `gx serve`'s start-up line counted the row under
   > `refused`, a number gx's own source defined as "the substrate was **not** written to".
   >
   > **What is different now.** On that road — the ledger already holds this row's leaf — the
   > recovery does not apply anything at all. A leaf in the ledger is evidence the apply completed
   > before the crash, because gx reaches the ledger after the apply returns, so the fingerprint the
   > rebuilt receipt needs is **read** off your substrate rather than produced by writing to it
   > again. The refusal is therefore a statement about a world this run did not touch, and
   > `refused` means what its own definition says.
   >
   > **What that costs you, stated as a change and not as a fix.** If your file *has* moved since
   > the crash — you restored it, an editor rewrote it, another process wrote it — gx used to write
   > the old change back over it and close the row. It now **refuses** and tells you the reading and
   > the ledger disagree. That is the safer direction and it is not the free one: a project that
   > used to come back on its own may now need you to look at it.
   >
   > **And the honest edge of the new sentence.** A digest that does not match says the document is
   > not the one the ledger witnessed and nothing more, so the refusal now lists the three causes it
   > cannot tell apart — the signing key, a world that moved, a journal naming a delta the commit
   > did not apply — instead of asserting one. The one case that *can* be established is: where your
   > project's recorded head names a different key, gx says so, because a signed head is evidence.
   > The old sentence asserted "the difference is the signing key" unconditionally, and the audit
   > read it back verbatim on a bed where both runs used a byte-identical key.
   >
   > **Two residues, declared rather than argued away.** (1) Between the read and the record gx
   > writes after it, a third party can write; the window is the same one this page files for the
   > roll-back's read (v0.4-p) and it is not closed here. (2) The read asks the adapter what the
   > object holds now and compares the result against a fingerprint the apply produced. For the
   > shipped filesystem adapter those are the same computation over the same bytes. An adapter for
   > which they are not would be refused where it used to be closed — fail-closed, and still a
   > behaviour this release has measured on `fs` alone.
   > (`crates/gx-engine/tests/crash_recovery.rs` has **thirteen** tests, two of which this release
   > rewrote because their fixtures had been insulated from the world by the very re-application
   > the audit was about)
   >
   > v0.5-t (2026-08-21, `req/38` §254, from the thirty-third adversarial audit `req/449`):
   >
   > 🔴 **Correction to v0.5-s above (the old text stands). The paragraph headed "What that costs
   > you" is about one of the two recovery roads and does not say so, and on the other road gx
   > still writes over your file and still does not tell you.**
   >
   > v0.5-s's cost paragraph reads: *"If your file has moved since the crash — you restored it, an
   > editor rewrote it, another process wrote it — gx used to write the old change back over it and
   > close the row. It now refuses."* The limiting clause is in the paragraph **before** it ("On
   > that road — the ledger already holds this row's leaf —") and is not repeated, so read on its
   > own the sentence claims something about every recovery. It is true of one of two roads. The
   > `CHANGELOG` entry for the same release limits it correctly; this page did not.
   >
   > **The other road, and what happens on it.** A crash between the apply and the ledger append
   > leaves a row the ledger holds **no** leaf for. gx finishes that commit by applying the delta —
   > that is what the road is for, and the commit really had not completed. The audit reconstructed
   > exactly that crash, let a third party write `THIRD PARTY` into the file afterwards, and ran the
   > shipped verbs: `gx repair --yes --json` answered `rc 0`, `repaired: true`,
   > `recover.resumed: 1`, `refused: 0`, `refusals: []`, printed nothing on stderr, and the file was
   > `two` again. `gx serve` started, answered `/healthz` `200 ok` with `ledger_agrees: true`, and
   > printed one note naming the road. A run under a key that had never signed this project did the
   > same thing. On the *other* road the identical third-party write is refused and the file is not
   > touched.
   >
   > **Why it is not simply fixed, stated as a limit and not as a plan.** To refuse, gx would have
   > to tell *its own unrecorded apply* from *somebody else's write*, and on this road it cannot.
   > The journal record that says the adapter was asked carries the transformation, the delta's
   > content address and a timestamp — no fingerprint. The only fingerprint the journal holds is the
   > one the plan was made against, and with an apply announced, a reading that differs from it is
   > exactly what a **successful** apply looks like (this is `req/78` §3.2 Λ4, and re-running the
   > compare-and-swap there is the mistake Λ4 exists to forbid). A post-apply fingerprint would
   > separate the two; the record for it does not exist, adding one changes the journal vocabulary,
   > and that is a decision with its own document rather than something to slip into a repair.
   >
   > **What v0.5-t changed was the telling — and v0.5-t said it changed all of it, which was not
   > true when it was written.** The sentence that stood here in v0.5-t read, verbatim: *"What
   > v0.5-t changes is therefore the telling, and it changes all of it. Every row that walks that
   > road now prints a sentence saying the delta was applied and that whether anything had written
   > to the object since the crash was **not** checked — including the sentence that if something
   > had, this run has overwritten it and cannot tell you."* It is kept here in its own tense
   > because it is the clearest thing on this page about how a false claim gets made: **the
   > paragraph twenty lines above it had just measured `gx repair --yes --json` printing nothing on
   > stderr, and the two sentences shipped in the same commit.** The sentence was written about a
   > repair that had been wired to `gx serve` and to nothing else.
   >
   > The count, taken from the source rather than from memory: **five** shipped verbs walk this
   > road — `gx verify`, `gx commit`, `gx undo`, `gx repair --yes` and `gx serve` — and a sixth,
   > `gx wrap`, inherits two of them, because the agent membrane drives `verify` and `commit`
   > itself. On the reconstructed crash above, with a third party's bytes in the file, four of the
   > five replaced those bytes and said nothing: `gx verify` answered **`rc 0`** with an `Admit` on
   > stdout and 0 bytes on stderr, `gx commit` `rc 1`, `gx undo` `rc 3` (its 689 bytes were a
   > compare-and-set refusal about something else), and `gx repair --yes` `rc 0` with
   > `repaired: true`, `refused: 0`, `refusals: []`.
   >
   > **What v0.5-u changes is that the sentence comes from whichever verb set the recovery off.**
   > Every row that walks that road **and is returned by the recovery** now prints it — on
   > **stderr**, under **that verb's own name** — from all five, and through `gx wrap` without
   > putting a byte on stdout that is not a valid MCP message. It says the delta was applied, that
   > whether anything had written to the object since
   > the crash was **not** checked, and that if something had, this run has written over it and
   > cannot tell you so. The wiring is arranged so that silence is the thing that takes work: the
   > three verbs that share one entry announce inside it, so a write verb added later is loud
   > without its author knowing the sentence exists, and a source census fails the build if a new
   > road appears on neither footing.
   >
   > 🔴 **The clause "and is returned by the recovery" was not in v0.5-u, and the next audit is why
   > it is here now.** What v0.5-u shipped read "Every row that walks that road now prints it …
   > from all five", and that quantifier was measured false by the audit that followed it — on the
   > half of the road v0.5-u had not looked at. `Engine::recover` builds its answer row by row and
   > returns it at the end; `Engine::resume` has **eight** fallible steps after the adapter has
   > applied the delta. When one of them raises, the rows already built are dropped with the error
   > and the row that had just written never became a row at all — so all five verbs had nothing to
   > print, and printed nothing. Measured on the same crash as above with the receipt archive made
   > read-only: `"THIRD PARTY\n"` became `"two\n"` on `gx repair --yes`, `gx undo`, `gx verify` and
   > `gx commit`, with **0 bytes** of sentence, and `gx repair --yes --json` answered
   > `repaired: false`, `recover: null`, `engine_open_failed.stage: "recover"` — a shape this same
   > binary defines elsewhere as "**Nothing was applied**".
   >
   > **This release repairs that half rather than the page.** The engine keeps what it had already
   > done when it raised — the rows it finished, and separately the rows whose delta reached the
   > substrate and whose commit it could not record — and all five verbs say so on stderr in their
   > own name, `gx serve` included. `gx repair --json` names those rows under
   > `engine_open_failed.applied_before_failure`, and its remedy on that road no longer contains
   > the word `refused`, because the word is this product's own for "nothing happened". The
   > sentence for this road does not say the row was *finished*: it says the delta was applied, the
   > record was not, and the row is still resumable. **What is still true and still a limit**: the
   > two facts a counter cannot carry are exactly as uncomparable here — no post-apply fingerprint
   > is recorded, so if a third party wrote to the object after the crash, this run has written
   > over it and cannot tell you so.
   >
   > A counter is **not** a substitute for the sentence, and only one of the four silent verbs even
   > had one. `gx serve`'s start-up line splits `resumed` into its four roads so the one that writes
   > (`apply_was_announced`) is a number you can alert on, and `gx repair --json` has published the
   > same number since v0.4. What a number cannot carry is *what could not be compared* and *that
   > this run may have destroyed somebody's bytes* — which is the whole of what is being claimed
   > here. If you recover a project after a crash and something else may have touched the object in
   > between, that number is the one to alert on and the sentence is the one to read, and the
   > escrowed inverse restores what the commit replaced — **not** what the third party wrote.
   >
   > **And a second thing the same audit found on the same road.** When the re-apply *fails*, the
   > row ends `Aborted(ApplyFailed)` and the roll-back is **not attempted**, because there is no
   > rebuilt inverse in a recovery that rebuilt nothing. An adapter that performs the change and
   > then loses the answer coming back — the commonest real failure — leaves a changed world with no
   > record of the change, and until v0.5-t `gx serve` counted that row under "resumed 1
   > transformation(s)" and said nothing. It now has its own count
   > (`recover.announced_and_aborted`) and its own sentence. `gx repair` has counted it apart since
   > v0.4; the two verbs now agree.
   >
   > **A third residue, named for the first time.** v0.5-s's residue (2) — an adapter whose read and
   > whose apply are not the same computation — is now also in the refusal's own text. The refusal
   > lists causes for an operator to rule out, and all of them could come back clean while this one
   > was true, because it was not on the list. It is now the fourth item, together with the fact it
   > implies: under that cause, re-running reads the same world and rebuilds the same payload, so
   > **no verb closes the row**. That is the shape this page files for a lost signing key, reached
   > with the key in hand and nothing damaged. Whether to add a verb that overrides the comparison
   > is a decision with its own document; declaring it is what this release does.

---

## The HTTP face is a loopback face

`gx serve` has **one** authorization check and it is a single static Bearer token, passed as a
start-up argument. It answers whether the caller holds this server's token and **nothing** about who
they are: there is no authorization layer in this build, `cancel` and `escalation` accept the actor
the request declares, and 43 T-7's owner guard has no enforcement point. The token is mandatory — a
server started without one refuses to start rather than answering every request with an error.
Accordingly the default bind is loopback (`127.0.0.1:8787`) and a `--bind` outside loopback is
**refused**; there is no override flag, because an override would turn the ruling into a checkbox
(M6H6-6 is registered so that a reviewer can rule one in, and until then the refusal is what makes
the condition observable). `--tls-cert` / `--tls-key` are refused for the same reason: an accepted
flag with nowhere to go would leave you believing the socket is encrypted.
**So: putting this face on anything but localhost is outside what this build does.** Terminating TLS
and performing real authentication and authorization in front of it — a reverse proxy or equivalent —
is the operator's responsibility, not this build's.
(`crates/gx-api/src/auth.rs` `DEFAULT_BIND` / `ABSENCE_NOTICE` / `bind_refusal`;
`crates/gx-cli/src/serve.rs::resolve_bind`; 44 §2.5 ASM-44-1; M5H6-4 adopted (a))

## What a receipt's read-set covers, and what it does not

A `CommitReceipt` can carry a **read-set**: the objects gx read in order to build the inverse it
escrowed. Two things about it are load-bearing and both are stated here rather than left to be
discovered.

🔴 **`unknown` and `false` are no longer the same bytes on a receipt** (**DR-46-26**, `req/38`
§258). The two paragraphs above this one — written for v0.5-e and kept — say twice that they were,
and that was the true and load-bearing limit until this release: a receipt whose `inverse_delta` was
`null` meant *either* "no tool undoes this change" *or* "the prior could not be read and this
deployment chose to take the effect anyway", and the remedies for those differ (the first is a
property of the change; the second is a posture an operator chose and can unchoose). A receipt now
carries `reversibility`, which is C-25's three values in the **signed** bytes, and the escrow row
answers `Undetermined` rather than `Unavailable` on the second road.

**What this does not fix, said in the same breath.** Nothing above it on this page moves: the
escrowed bytes are still bound to the attested object and not to the restore call's own target; a
read face declared in one file and an effect declared in another are still two files; and the field
says *which* answer this deployment reached, not whether reaching `unknown` was the right thing for
the deployment to allow. A commit receipt rebuilt by the crash-recovery road carries an answer **derived from the
journal**, and the two things it derives it from are the escrow row's status and the escrow's own
record of what it read.

> 🔴 **What this paragraph said before, and why the correction is worth the space** (R35,
> `req/470` M-01). Until this release it read: *"A commit receipt rebuilt by the crash-recovery
> road carries the answer the **filed** receipt carried, and where no filed receipt survives, the
> rebuild carries none and refuses rather than inventing one."* That describes a first
> implementation which the lane that wrote this paragraph had **already measured and thrown away**
> — its own engine doc heads the function *"The first shape of this was wrong, and the beds said
> so"*, and the reason is structural: R13 closes the row from a filed receipt, when one exists,
> *before* the rebuild is attempted, so the rebuild road is by construction the road on which
> **there is no filed receipt**. A description whose condition never holds describes nothing. Both
> sentences — the discarded model and the code that replaced it — shipped in the same commit.
>
> What ships instead: the verdict is projected from the `InverseStatus` the escrow row carries
> (`Unavailable` → `false`, `Undetermined` → `unknown`, and the escrowed cases → `true`), and the
> read-set is read back from the journal's `InverseEscrowed.reads`. Both are journalled, so the
> rebuild reproduces what the commit signed rather than recovering it from a document that is not
> there. `RECOVERY_REBUILD_DISAGREES` is unchanged from R13 and is a different check: it fires when
> a rebuilt payload does not match the leaf the ledger already holds.

🔴 **A receipt now says where the determinism ends — and the sentence is narrower than it sounds**
(**DR-46-28**, `req/38` §255 ruling 4, `req/459`). Every receipt carries a `determinism_boundary`
with four values: `deterministic_replay`, `llm_originated`, `mixed` (which names its two stages),
and `unknown`. Read the claim exactly:

* **`deterministic_replay` means "replaying the same input yields the same verdict" and nothing
  wider.** The engine has a clock in it, generates keys, and walks a filesystem whose order it does
  not fix. None of those reach the verdict; all of them are still there. A buyer who reads this
  field as "gx is deterministic" is reading something gx does not say.
* **`llm_originated` is a record of origin and not an assertion about the output.** It says a model
  produced the material. It does not say the material was right, was reviewed, or was gated.
* **`unknown` is a value here, not a silence** — the value gx writes for the input-generation
  stage of a deployment that has **declared nothing**. gx does not observe how an input was
  generated: the deployment declares that, in its catalogue's `$determinism_boundary` slot.
  🔴 **DR-46-33** (`req/38` §413) now joins the two: an optional `InputStageDeclaration` carries the
  declaration into the engine, the engine overrides it to `llm_originated` when the transformation's
  actor is an `Actor::Agent` (a model origin whatever a static file declared), and the **result** is
  journalled on the `Planned` record so 43 §7-3b's rebuild reproduces the boundary without the actor
  (which Σ does not carry) or the catalogue (which `gx-engine` does not depend on). So a receipt's
  boundary now names the deployment's declared stage where one is registered — `mixed` naming
  "input generation: deterministic_replay / verdict derivation: deterministic_replay" collapses to
  `deterministic_replay` — `llm_originated` for an agent-authored change, and `unknown` only where a
  deployment declared nothing and no agent authored the change (and on the fail-open road where no
  gate ran, 43 T-4e). Where the input generation is still `unknown` the field tells you which side
  of gx's own arithmetic a change was on and does not tell you whether a model wrote the input.
* **One value never appears on a receipt at all.** `llm_originated` as a whole-transformation value
  would say gx derived the verdict from a model; gx derives verdicts, so the schema refuses it. It
  exists for the declaration face, where a tool class gx never gates can carry it honestly.

The field is not read back into any decision: a boundary that certified a derivation while feeding
it would be circular, and the suite scans the gate for the name and counts the gate's declared
inputs rather than promising it here — `crates/gx-witness/tests/boundary_attest.rs` has **eleven**
probes, one per value of the taxonomy plus the wire, the vocabulary and that scan.

**It is gx's read, not the agent's.** The membrane is on the effect, so `resources/read`,
`resources/list`, `prompts/get` and their siblings pass through the adapter untouched and nothing
about them is kept — `crates/gx-mcp-wire/src/method.rs` classes them `Passthrough` and says so in
the table itself ("Reading is not calling"). What the read-set attests is the prior of the object
this change is about. **So it does not tell you whether one agent read what another agent wrote**,
and a claim that attesting a read-set makes *selective* undo well-defined does not follow from this
field. That claim needs the agent's read traffic, which is filed as **DR-46-25** and whose cost has
not been measured at a single point. Until DR-46-25 ships, selective undo is a roadmap word here and
nothing on this page or in the product should read otherwise. (**DR-46-24(A)**, `req/38` §236
ruling 3.)

🔴 **`read_set: null` is not one fact, it is three, and the bytes do not tell them apart** (R35,
`req/470` L-04). The constructor's own doc defines `None` as *"an escrow that read nothing"*, and
that is one of the three roads to those bytes. The others are that **no `InverseEscrowed` record
for this transformation exists**, so the rebuild defaulted an empty list, and that the record was
written by an **older journal** whose format had no `reads` field at all. "I read nothing", "I have
no record of what was read" and "this journal predates the question" are three different
statements about a receipt, and today they are the same absence.

Which matters most is the first: on every shipped adapter today the read-set is **always** empty,
because the outcome constructor fixes it to an empty list on both arms — so `null` on a v0.1
receipt carries approximately no information, and reading it as "gx confirmed this escrow read
nothing" is the misreading this paragraph exists to prevent. Note the contrast with the field
beside it: `undetermined` is written whenever it is `true` and defaulted to `false` otherwise, so
*that* absence has a discriminator and this one does not. Whether the absolute absence should be
given its own spelling on the wire is **filed as a decision request**, a sibling of DR-46-13, and
is deliberately **not** decided here — a fourth value invented inside a repair lane is exactly the
kind of vocabulary change 42 §3.13 asks to be taken as a decision rather than slipped in. (The
number is assigned when the request is ruled on and not before: `req/38` §264 ruling 2 had to
un-pick one lane's guessed number after two documents guessed differently.)

**The guarantee has two strengths and the receipt says which one it is carrying.** Up to five
distinct objects the receipt carries the entries themselves and answers "was this object read?"
**from the receipt alone**; past five it carries a Merkle root instead, and the same question then
needs the entries from beside the receipt — the root alone is a digest, and a digest with no
preimage decides nothing. The receipt names which of the two it is, as a tag inside the field
(`G3` for the entries, `G4` for the root), because a guarantee that changes silently at a threshold
is worse than a uniformly weaker one. Measured: the entries cost about 102 bytes each, so a receipt
holding six is roughly 2.3× a plain one, while the root costs 52 bytes whatever the count
(`crates/gx-witness/tests/d24_read_set_cost.rs` has **five** probes, and the four counts 1/5/6/32
are pinned in the last of them). The inclusion path is **not** stored anywhere: it
is a function of the entries and is derived when a verifier asks (86 µs for 32 objects), because
storing every path costs more than storing the entries it is derived from.

**The scope of a fingerprint is on the wire now, and one consumer has not been rewired yet.** A
receipt carries `fingerprint_scope`, so a reader can see what the pre- and post-state fingerprints
covered rather than only their digests. The undo road's own compare-and-set still compares digests
directly, which is what it had to do while the scope was missing; moving it onto
`Fingerprint::cas_eq` is a follow-up and is filed as such. (**P2**, `req/350` §7-4.)

## The conditions the v0.4 GO is stated under

The outside-the-box GO (Model A) declared for v0.4 is a **conditional** claim, and the eight
condition clauses are part of the claim rather than a footnote to it — 9p measured nine times while
Windows-native/OneDrive/SMB is measured zero times; Model B (an adversary who can write `.gx/`) is
answered by external artifacts, not local detectors; NoArchive effects cannot be undone; legacy
projects are reachable only through gate (2); the 14 ms residual window of H-04 is filed; `/healthz`
costs a writer more under load; the mode-change road can still cost a receipt; and the open M/L items
of the seventeenth audit are the next release's input. They are enumerated in full in `req/38` §195
(clauses (1) through (8)), which is the maintainer-side ledger this page's citations point into.

## Installing a policy pack does not make a substrate safer

It makes it **judged**, and those are different sentences. Before a substrate has a shipped
pack, gx has no statement about it at all, so every change to it is refused by Cedar's
third rule — nothing satisfied, therefore deny. That refusal looks like protection and is
not: it is the absence of an opinion, and it names no rule you could cite, appeal or
audit.

The three packs shipped for `fs`, `git` and `mcp` each open with a
`<substrate>-permit-default`. Installing one of those therefore **widens what is admitted**:
the pack's forbid takes over the surface it names (`/etc`, `refs/tags`, `file:///etc`), and
everything else on that substrate becomes admissible where it previously was not. That is
the trade the pack is for, and it is a trade — you get rules you can name, in exchange for
a default that says yes.

The `postgres` pack is deliberately not that shape. It ships one forbid and no permit, so
installing it changes what is admitted by **zero**: a write to a business table falls to
Cedar's third rule after the pack exactly as it did before. What changes is that a write
into `pg_catalog` or `information_schema` is now refused by a named statement instead of by
the absence of any. The reason for the asymmetry is in that pack's header: `fs`, `git` and
`mcp` each have a small universal thing worth protecting and a bounded remainder it is
honest to admit; postgres's real damage surface is one deployment's own tables, which a
shipped pack cannot know a row of.

So, before you install a pack, read which default it declares — `policies/PACK_FORMAT.md`
F3 requires every pack to say. A permit-default pack is a starting point that says yes by
default. A deny-default pack is a starting point that says nothing by default and refuses
one named thing loudly. Neither is a policy for your deployment; both are the floor you
write yours on top of.

## What a policy can be about at all

A statement in a pack can only speak about what the request carries: the locator, the
substrate, the actor's key, the change context, the order, whether an inverse could be
built, and how much evidence of which kinds was shown. Nothing else is in reach —
**including the clock**.

That last one is worth stating plainly, because it is the limit people ask about most. You
cannot write "only during business hours", "no more than N changes an hour", or "expire
this permission on Friday" as a gx policy. It is not that the rule is unimplemented: the
request a policy is evaluated against carries no time value at all, so there is nothing for
such a clause to compare against. A gx policy is a statement about *what* a change is, not
about *when* it arrives.

Time is not entirely absent from the layer — the transformation being judged carries the
timestamp it was planned at, and an invariant written in Rust receives it. But the wall
clock at the moment of judgement is not passed down, so even there the question a window
needs ("is now inside it?") cannot be asked. Time-scoped rules are therefore a question for
a layer above this one, and are filed as such rather than half-shipped here.

## A registry that holds a receipt is a distribution path, not a verification path

`DR-46-29` fixes the two names an OCI registry needs before anything can push a receipt to
one at all, and fixes nothing else — the verb (`gx receipt push --oci`) is not implemented
by this clause, only its shape, because a name that changes after artifacts already carry
it is a breaking change with no version field to absorb it, while a verb that ships late is
just a verb that ships late (`req/461` §1, ruling (c)).

**The artifact type, and the layer's own media type, are the same fixed string:**
`application/vnd.tracefold.receipt.dsse.v1+json`. It is deliberately not
`application/vnd.dsse.envelope.v1+json` — the generic DSSE-over-OCI type cosign and the
in-toto attestation tooling already use — because that type is a promise about the JSON
shape underneath it (`secure-systems-lab/dsse`: camelCase `payloadType`/`payload`/
`signatures`), and [`DsseEnvelope`](/crates/gx-witness/src/dsse.rs) does not keep that
promise: its fields are `payload_type`/`payload`/`signatures`, snake_case, because 44 §2.2
fixes the JSON face of 42 §3.10's `Receipt` on its own terms rather than on the upstream
spec's. A tool that trusted the generic type would parse valid JSON and read absent fields
without an error to show for it — the wrong kind of silence for a security-relevant
identifier to produce. `application/vnd.glovrex.receipt+dagcbor`
([`RECEIPT_PAYLOAD_TYPE`](/crates/gx-witness/src/dsse.rs)), fixed since 42 §3.10, is a third
and different string: it names the *payload's* encoding inside the envelope (canonical
DAG-CBOR) and travels inside the signed bytes. `tracefold` names the outside, the string a
registry and a stranger's tooling see before they have any key to check a signature with;
`glovrex` names the inside, the string this codebase's own signer and verifier already
agree on and is not free to change here. Nothing about DR-46-29 touches the second string.

**The subject a receipt attaches to is a digest reference, never a tag:**
`<registry>/<repository>@sha256:<hex>`. This is not a style preference — the OCI 1.1
Referrers API defines `subject` as an immutable descriptor, and a tag is a pointer a
registry lets move, so a tag-named subject would leave the referrer relationship pointing
at whichever manifest happens to answer that tag's name at pull time rather than the one
that existed at push time. A receipt's whole claim is about one transformation; a subject
that can be repointed underneath it would make the OCI attachment lie in exactly the way a
receipt itself is built not to.

**Two facts about how a puller finds the receipt back were not assumed into this fixation
and were checked against the tools' own source instead of a description of them**
(`req/461` §2's lane-first-move): the OCI 1.1 Referrers API is not the only road — a
registry that has not implemented `GET /v2/<name>/referrers/<digest>` (or answers it with
anything other than an `application/vnd.oci.image.index.v1+json` body) is not unreachable,
because `go-containerregistry`'s `fetchReferrers` (the function both `cosign` and `oras`
build on) falls back to the tag-scheme index (`<algorithm>-<hex>` as a tag) automatically
when the API leg fails, and `oras attach`/`oras discover` expose the same choice explicitly
through `--distribution-spec v1.1-referrers-api|v1.1-referrers-tag`. Neither cosign nor
`go-containerregistry` requires the tag scheme — the claim this fixation does **not** make is
"referrers only works by tag"; the claim it does make is "a puller that only tries the API
leg, on a registry that only serves the tag leg, finds nothing and must not read that as
the receipt being absent." And the push side's real syntax is `oras attach --artifact-type
<type> <subject>{:<tag>|@<digest>} <receipt-file>:<type>` (not `oras push`, which sets no
subject at all) — `attach` is the ORAS verb whose whole job is producing a referrer, and is
the form the two composite actions this clause ships (`.github/actions/oci-receipt-attach`,
`.github/actions/oci-receipt-discover`) are built around.

**What pushing does not do**: it does not make a receipt more true. `gx receipt verify
--offline` reads a signature, a canonical CID, and — given a checkpoint — an inclusion
proof; none of those three checks consult a registry, and none of them get easier or harder
for having one nearby. A registry that goes down, changes ownership, drops the tag-scheme
index, or never gets queried at all leaves every receipt it ever held exactly as verifiable
as it was the moment it was signed, because the verification path was always the offline
one and the registry was only ever a place a stranger could find the bytes without asking
this project for them first. This is the same clause `docs/LIMITS.md`'s Cloud-storage SCOPE
already states for a different transport: **a place a receipt can be found is not, and does
not become, the way a receipt is checked.**

---

`docs/TUTORIAL.md` ends on this page for the same reason it opens the list above: a
walkthrough that stopped at the parts that work would be a walkthrough that let you find
the parts that do not on your own, later, on something that mattered.

---

## v0.5-u — a change a person allowed can have its receipt re-issued, and the journals written before this release still cannot (2026-08-21, DR-46-31)

**What was wrong.** When gx cannot build an undo for a change, it does not commit it quietly:
it escalates, and a person has to allow it by name and with a reason. That ruling is signed,
and the digest of it is what the commit's receipt is issued under. Until this release the
digest was the one thing about the ruling that was **never written to the journal** — the
record carried who ruled, what they decided and why, and not the value taken over those three.
So a gx that rebuilt the project's state from the journal alone, which is the only thing
`gx repair` has, produced a state that named the person's `Admit` beside the digest of the
*escalation* the gate had raised. The rebuilt receipt therefore did not match the one the
ledger had witnessed, and `gx repair --yes --reissue-receipts` answered **`world_moved`** — a
sentence about a substrate that had not been touched — for **every** commit that had ever been
escalated. Since a commit is escalated exactly when no undo could be built for it, that was
every irreversible change in the project: the rows an operator has the most reason to want a
receipt for were the rows whose receipt could never be restored.

**What changed.** The journal record now carries the ruling's digest. Rebuilding reaches the
value it always needed, the re-issue reproduces the leaf, and the verb works on escalated rows
the way it has worked on ordinary ones since R8.

**What did not change, and this is the part to read before trusting it.** A journal written by
an earlier release does not have the field, and **gx does not repair those rows**. It could
have: the ruling's three recorded facts are the whole of what the digest was taken over, so the
value is re-derivable today, and a version of this fix that re-derived it would have made every
old project's escalated rows re-issuable at a stroke. It is not done, because a re-derived
digest would be presented as *the digest that receipt was signed under* while actually being a
fresh computation — and the two part company silently the day the ruling's shape or its
canonical encoding moves, on journals already sitting on disk. gx would then be asserting
agreement between a receipt and a state that had never read the same value. So on a
pre-v0.5-u journal the re-issue still answers `world_moved` for an escalated row, and the
honest reading of that answer is narrower than the words: it means **this project has no record
of the digest that receipt was issued under**, not that anything moved. The filed receipt, where
one still exists, is unaffected and verifies exactly as before; what cannot be reconstructed is
a receipt that was lost.

**Where the boundary is.** New commits ruled on by a person under this release and later: the
re-issue works. Commits ruled on before it: it does not, permanently, by choice. There is no
migration, because the only migration available is the derivation this clause declines to make.

## `gx confine` restricts writes, on Linux, for the process it starts — and four things it does not do

`gx confine -- <cmd>` asks the kernel (Landlock) to refuse writes the catalogue has not
admitted, and then becomes `<cmd>`. It is the first thing in this build that stops a write
rather than recording one. What follows is what it does *not* stop, stated here because a
mechanism that refuses something is read as refusing everything.

**It restricts the write face of the filesystem and nothing else.** The rights handed to the
kernel are Landlock ABI 1's write set — `WriteFile`, `MakeReg`, `MakeDir`, `RemoveFile`,
`RemoveDir` and the four device/socket makers. **Reads are not restricted**: a confined command
can read every file the user running it can read, including this project's own store. **Execution
is not restricted**: it can start any program it can reach. Two consequences of the ABI in
particular: `rename` is Landlock's `Refer` right, which arrives at ABI 2, so a rename is not
covered; and a write to `/dev/null`, to a tty, or to any other device outside the granted paths is
refused like any other write, which is a real thing to trip over when the confined command is a
shell script.

**It is Linux-only, and there is no partial road.** Landlock is a Linux facility. On any other
platform `gx confine` refuses and runs nothing, rather than running the command unconfined — a
build that fell back would be reporting a confinement that does not exist. Windows has its own
mechanisms (job objects, AppContainer); they are not this, and this build does not use them.

**It does not stop a process that is not inside it.** Landlock narrows what the calling process
and its children may do. It does not change what *anyone else* on the machine may do. A process
running as the same user that `gx confine` did not start is unrestricted, and a confined command
that can reach such a process — over a unix socket, or by `ptrace` — can ask it to perform the
write the kernel refused. The process face of this reaches exactly as far as `no_new_privs`: the
confined command cannot gain privileges it did not start with. It is not an execution allow-list,
and there is no syscall filter.

**The paths come from the command line, not from the catalogue.** The catalogue decides *whether*
a named tool may write at all — a tool it does not declare gets no writable path, whatever was
asked for — but it holds no filesystem paths, so *where* is `--allow-write`'s answer and not a
file's. Every run says so: the report carries `write_targets_are_declared: false`. It also names
one case it cannot close: if this project's `.gx/` sits beneath a granted writable path, the
confined command can write the store, because a Landlock rule grants access beneath a path and
ABI 1 cannot carve a hole in one. The report says that too, under `gx_store_exposed`, rather than
printing "confined" over it.

**And line 2 above still stands.** An attacker with root or kernel privilege writes around all of
this exactly as before. `gx confine` narrows what an ordinary, unprivileged, same-user process is
able to do; it does not change who wins against the kernel.

## A receipt this product issued in August 2026 does not verify against this build

**The claim this qualifies.** The third pillar is that a receipt can be re-verified offline, by a
stranger, without the issuer. Without the issuer means without the issuer's *next release* too. That
is not true today for the oldest receipts this project has produced.

**What is measured.** `crates/gx-witness/tests/frozen_receipt_corpus.rs` has **six** probes and
holds a receipt, a checkpoint and a public key that this project issued on 18 August 2026, byte for
byte. On every floor run those six assert: the DSSE signature over the frozen bytes still checks out, the
payload **still does not decode**, the frozen bytes carry none of the members that were added
after they were written, and — added in R39 — **every member this section declares is still required
with no `serde` default in the current schema**, and the set this section names is the set the suite
checks. That last pair is what makes this section an alarm rather than a note: a build that widened
one of the two members would leave a decode-refusal probe green, because `serde` names one absent
member per refusal.

`gx receipt verify` answers exit **7** for this document, which is the word for *refuted*, about a
file nobody has touched. That sentence is driven where the binary is:
`crates/gx-cli/tests/r39_frozen_receipt_verdict.rs` has **five** probes, which hand the frozen
artefacts to the shipped binary the way a third party would and assert the 7. A control changes one
byte of the specimen and asserts the answer about it changes — the exit stays 7, and what moves is
the signature check inside it — so the number on this page cannot be produced by a probe that never
opened the file. The same suite counts the specimen's copies in the tree, which must be one.

**Why.** Two members were added to `ReceiptPayload` as required, with no `serde` default —
`fingerprint_scope` (P2) and `determinism_boundary` (DR-46-28). Either alone refuses the file. The
same-shaped change was made correctly one and two errata later (`verdict_digest` in DR-46-31 and
`read_set` in DR-46-34, both `Option`), which is what makes this a mistake rather than a trade-off.

**Why it is not simply fixed by making those two optional.** It was tried, twice, in R38, and the
floor refused both. A `serde` default restores decoding and silently changes the value, because
`ReceiptPayload::ledger_digest` re-encodes the *struct*: the leaf moves, and the receipt is then
`refuted` rather than unreadable, which is a worse answer. `Option` with `skip_serializing_if`
removes those two from the re-encoding and **the re-encoding still does not match the bytes that
were signed**, because `read_set` and `reversibility` are written as explicit nulls on purpose —
DR-46-34 made a null the fourth spelling of an absent read set, and canonical CBOR is right to
distinguish "absent" from "present and null". Making the boundary optional also contradicts
`req/459` ruling 3, which makes `unknown` a first-class value precisely so that one fact has one
shape.

🔴 **R39 — this paragraph used to carry a byte count and no longer does.** It gave the difference a
size in bytes. That number was measured on the second of the two builds above, which was then
withdrawn, and no instrument in this tree can re-derive it: the re-encoding it describes
begins with a decode, and the decode failing is the limit itself. A public number nothing re-derives
is a number that becomes false in silence, so it came off the page rather than being left to be
trusted. What is measured in its place is the size the limit does have — the specimen carries
**eleven** members and this build's `ReceiptPayload` names **eighteen** — and both halves of that are
read off the artefacts on every run (`r39_frozen_receipt_verdict.rs`, `tools/receipt_generation_gate.mjs`).

🔴 **`req/919` W5 (2026-08-29) moves the count again, seventeen to eighteen: `payload_version`
(F7, `req/868` R-868-6) landed.** Unlike the four members below that widen how many old, real
receipts a decoder can still open, this one is `Option<u32>` with `#[serde(default)]` -- every
receipt signed before this field existed still decodes, `None`, exactly the reading `confinement`'s
and `catalogue_hash`'s absence already have on this struct. What changes is that a receipt this
build issues, decoded by a reader with no access to this repository's history, now names its own
schema generation instead of leaving F7 unanswered.

🔴 **`req/901` (2026-08-26) corrects two things in the sentence above, and the second is the worse
one.** The count said **fifteen**; the struct has **seventeen**, and had since `confinement` and
`catalogue_hash` landed. But the clause claiming both halves were "read off the artefacts on every
run" was **not true of the second half**: `r39_frozen_receipt_verdict.rs` asserts only that the named
count exceeds the carried one, which stays green at any number above eleven. So a page that said it
was measured was carrying a number nothing measured — the same failure mode as the incident this
whole section exists to describe. `tools/receipt_generation_gate.mjs` now counts the struct's members
directly and the specimen's from byte 0 of its DSSE payload (`0xab`, a canonical CBOR map header),
and fails the run on a stale statement of either. The clause is true as it now reads because that
instrument exists; it was not true when it was written.

**So the limit is one level down from where it looks.** A signed, archived document's ledger leaf is
re-derived from a struct whose canonical form moves with the schema, so every member ever added has
already moved every historical leaf. Closing this means deriving the leaf from the bytes that were
signed, or recording it, and that is a change to 42 §3.10 and the DR-46 series rather than to a
decoder.

**What holds in the meantime.** The signature and the anchor are unaffected — a holder of the 2026-08
artefacts can still show that this project signed them, and that the checkpoint is the one it
published. What they cannot do with this build is have `gx` confirm the inclusion proof. There is no
version of `gx` that can, and this section will say so until there is.

🔴 **Update — the level below has been repaired, and this section is now narrower than it was.**

The paragraph above says closing this "means deriving the leaf from the bytes that were signed", and
that it is a change to the data model rather than to a decoder. That change has landed. `gx` now
derives a receipt's ledger leaf from the bytes that were signed, on every road that verifies one, so
a member added to `ReceiptPayload` no longer moves the leaf of a document written before it. The
`crates/gx-witness/tests/leaf_from_signed_bytes.rs` has **five** probes: the two roads agree for a
receipt this build issues, the bytes road states a leaf for a document the value road cannot even
open, the leaves of both frozen specimens are pinned to constants, those two constants are different
numbers, and the inclusion check is read at its source to confirm it takes the bytes road. The leaf
of this very specimen is one of those pinned constants, and it was derived from bytes this build
cannot decode — which is the whole point, and something no road in this workspace could do before.

**What still holds this document out is the decode, alone.** The two members added as required with
no `serde` default are still required, so `gx receipt verify` still stops before it reaches the
inclusion check and still answers exit 7. The paragraph "why it is not simply fixed by making those
two optional" was written when the leaf moved underneath any such fix; that objection is now spent,
and what remains is a decision about the two members' shape rather than a defect one level down.
Until that decision is made and landed, this section stands.

**Two sentences above are therefore no longer true as written**, and are kept rather than edited
because they are the record of what was known when: "There is no version of `gx` that can" — this
one can derive the leaf, and cannot decode the payload — and the reading that a `serde` default
would "silently change the value", which was true of the build that measured it and is not true of
this one.

## A receipt says what was read, what was written and when — and never by whose authority

**The claim this qualifies.** `gx receipt coverage <FILE>` answers four questions about a receipt:
what was read, what was written, when, and by whose authority. The fourth one is answered
`unknown` on **every** receipt this build can produce, and it is not a defect being worked around —
there is nothing in a receipt to answer it with.

**What is measured.** `ReceiptPayload` has eighteen members (`req/901`, 2026-08-26: this said
fifteen, counted by hand at a time when it was true, and nothing recounted it when two more landed —
`tools/receipt_generation_gate.mjs` now derives it; `req/919` W5, 2026-08-29, moved it again from
seventeen to eighteen when `payload_version` landed). `key_id` is the id of the key that
**signed**, which is a different question from who authorised the change; the actor lives on
`Transformation.actor`, and a receipt carries `transformation: TransformationId` — a join key,
not the transformation. So a reader holding one receipt cannot reach the actor even to under-read
it. `crates/gx-witness/tests/p1b_coverage_totality.rs` has **four** probes and asserts the
`unknown` on both receipt kinds, over every receipt document in this tree rather than over a
handful chosen by hand; the reason word it prints is `actor_not_in_receipt` rather than a generic
absence.

**Why it is not simply fixed by reading `key_id`.** A verifier that answered "by whose authority"
from the signing key would be reporting a key as a person, and would answer it *the same way* for a
change an agent made under a delegated key and for one a human made at a terminal. The honest place
to take this is the agent's own chokepoint, which is a route question and not a receipt question:
the face-level table (`gx attach`'s `coverage`) says `only-declared` for this question when a route
is wrapped, because what reaches gx there is a `--actor-key` flag somebody wrote down.

**What holds in the meantime.** The other three questions are answered from the signed bytes and
carry their own bound with them: the read column names what it does not cover (the agent's read
traffic), the write column carries the scope its fingerprints were taken over, and the `when`
column says that what is fixed is the **order** and not the time, because no clock is inside the
signature (E-M2-6).

## The attach-face specimen frozen in August 2026 does not close the blindness the older one does

**The claim this qualifies.** `crates/gx-cli/tests/p1b_attach_face_frozen.rs` has **three** probes
and holds two receipts, a checkpoint, a public key and an attach answer that this project produced
on 22 August 2026, byte for byte, on a project placed by `gx attach`. On every floor run they
assert that both specimens still verify offline, that the files are the ones that were frozen and
not fresher ones, and that the project they came from was attached by this binary.

**What that is worth, and what it is not.** The value of the 2026-08-18 corpus one section up is
that **the binary under test did not mint it**: a change that moves what the encoder writes and
what the decoder requires, in the same commit, is invisible to fixtures that move with the code.
These specimens were minted by today's binary, at the start of the lane that wrote the suite. They
therefore do **not** close that blindness. What they close is drift *after* that point: from here
on, a change to how an attach-face receipt reads has to make this suite red first.

**Why it is left standing rather than strengthened.** The strengthening is time — a specimen is
worth what it is only once the build that made it is old. Saying so here is the alternative to
letting a suite that reads "frozen specimen" be taken for the corpus `req/38` §294-2 (b) asks for,
which is a stronger claim than this lane can make and is still open.


## Taking gx back out of a config puts the entry back, not the file

**The claim this qualifies.** `gx wrap --detach-config <PATH> --server-name <NAME>` undoes
`--adopt-config`: the entry runs the command it ran before, read back out of the wrapped `args`
themselves rather than out of any saved copy, which is why it works on every adoption this binary
has ever written. `crates/gx-cli/tests/p1c_detach.rs` has **eighteen** probes, and among them measures that the machine check
(`--check-config`) returns byte-identical JSON, under the same exit status, before an adoption and
after a detach.

**What does not come back.** The **file** does not. `--adopt-config` re-serialises the whole
document when it runs, and that normalises the indentation and drops the trailing newline; both are
gone before a detach is ever invoked, so no later operation can restore them. Every detach says so
in its answer (`not_restored`), on the runs that worked as much as on the ones that did not. An
entry that had no `args` member at all comes back with an empty one, because an adoption writes that
member onto every entry it touches and does not record which entries lacked it.

**Key order does survive, and this project does not control that.** `serde_json` is built here with
`preserve_order`, so an adoption keeps the operator's key order at every depth — measured, not
assumed. But the feature is switched on by a **dependency** rather than by this workspace's own
manifest, which makes it a property gx inherits and could lose without touching a line of its own
code. `the_document_keeps_its_key_order_through_a_round_trip` measures the property itself for that
reason: if it ever stops holding, that probe goes red rather than the sentence above quietly
becoming an understatement.

**What gx will not do.** It will not write back a preserved copy of the document, and it keeps no
such copy to write. An operator who adopts a config, then edits it for a month, then detaches, keeps
the month of edits: the detach touches `mcpServers.<name>` and reads nothing else. The cost of that
choice is the paragraph above — a reverse operation that restored bytes would have to overwrite
those edits to do it, and "reversible" is not worth a product that eats an operator's work to prove
it. Where an entry runs `wrap` in a shape this binary did not write, the detach **refuses** (exit 1)
and names what it could not read, rather than guessing and writing the guess into the document.

**What it does not touch at all.** `.gx/` and everything in it. That is not a limitation of the
detach, it is the point of it: the receipts issued while gx stood in front of the server verify
offline afterwards, and `receipts_issued_while_attached_still_verify_after_a_detach` measures that
they verify to the same bytes either side of the operation. No verb of this binary removes a
declared path, so there is no word for a removal in the detach's vocabulary either.

## A receipt says whether the kernel confined the run, and that sentence is a report and not a proof

A receipt issued by this build carries a `confinement` context: whether the process that produced
it was held by a Landlock ruleset, and which ruleset. It sits beside `enforced` rather than inside
it — gx checking a change and the kernel holding the process that made it are two different facts,
and all four combinations of them happen. What follows is what the field does not establish.

**The receipt reports a measurement it did not take.** `gx confine` asks the kernel, reads the
answer, and hands it to the process it becomes through an environment variable. The `gx commit`
that later writes the receipt trusts that variable. It has no way to re-check: by then the ruleset
is a property of the process, not something a later call can enumerate. So anything that can set a
variable in gx's environment can put `kernel_confined: true` on its own receipts. This is the same
trust boundary line 2 of this page already states — an attacker with that much access writes around
gx anyway — but the field's name invites a stronger reading, so the weaker truth is written here.

**A run that says `false` is saying something, and a receipt that says nothing is older.** Every
receipt this build issues carries the context, and a process nobody confined says
`kernel_confined: false`. An absent context means the bytes were written before this build existed.
The two are not the same sentence and the schema keeps them apart.

**What "confined" covers is what the section above covers.** The bit is the write face of the
filesystem, on Linux, for the process and its children. Reads, execution, `rename`, and any
same-user process the ruleset never touched are exactly as unrestricted as they are there. A
reader who takes `kernel_confined: true` for "this process could do nothing it had not declared"
is reading past every paragraph of that section.

**One thing this did not break.** The context is an optional member, so a receipt written before it
still decodes — and that was measured rather than assumed: the serde default beside it was taken
away and the compatibility probe stayed green, so it is the `Option` doing the work and the
attribute is the intent written down. It is not a third document added to the limit two sections up.
What it does move is the digest of any payload this build re-encodes, which is the same migration
every added member has made and which that section is about.
## A receipt is served while the same server refuses every question about the tree it came from (2026-08-22, R40, `req/553` L-02/L-01)

**The claim this qualifies.** `GET /healthz` tells a caller whether this deployment is well, and the
four ledger routes refuse when it is not. Cut a project's last committed frame under a running `gx
serve` and that is exactly what happens: `/healthz`, `GET /ledger/proof`, `GET /ledger/consistency`
and `GET /ledger/checkpoint` all answer **500 `LEDGER_DISAGREES`**, because no statement about
*which* tree this is can be honest while the journal and the ledger describe two.

**What is served anyway.** `GET /receipts/{tid}` answers **200** in the same second, with the same
bytes it answered before the cut, and `GET /candidates/{id}` answers 200 as well. Nothing in either
response says the project it came from is refusing every other question about itself.

**This is deliberate, and it is the promise made elsewhere on this page.** A receipt is a signed
statement about one transformation. It does not become false because a later frame was cut, and
`gx receipt verify --offline` proves it against the receipt's own bytes without asking this server
anything. The refusal `gx` prints for an absent journal says so in as many words — *"`gx repair`
reads the ledger, the commit receipts and the recorded head out of their own files"*, and *"`gx
receipt verify --offline` still proves what was committed"*. Putting the ledger gate in front of
these two routes would take that road away from the offline verifier and close nothing: **issuing is
not serving.**

**What gx does not do about it.** *(🔴 Stale as of the record two paragraphs below: the first
sentence of this paragraph is no longer true of `GET /receipts/{tid}`, which now carries a
`server_health` member. It is left standing, unedited, because the reasoning it gives is what makes
the change legible as a falsifier being honoured rather than a decision quietly reversed. Everything
it says about headers, about `BUSY`, and about the other responses is still true.)* It does not add
a `degraded` member to these responses and it does not add a header. 44 §2.2 fixes the response shape, `wire_census.rs` holds it, and DR-44-9 makes a
receipt view stand *beside* the document byte-for-byte; `BUSY` carries `Retry-After` and no other
code carries a header. So the limit is **declared here and driven**, rather than repaired by
widening the wire: `crates/gx-cli/tests/r40_serving_routes.rs` has **two** probes, and the first of
them asserts `/healthz` = 500 and `GET /receipts/{tid}` = 200 on one server at one instant, with the
served bytes identical either side of the cut.

**How a caller learns the difference.** By asking `/healthz`, which is outside the bearer guard and
answers `status: "degraded"` with a `status_reason` naming the way out. 🔴 **The condition under
which this answer stops being good enough is written down in advance**: if any consumer — a GUI, an
SDK sample, an integration — is observed rendering a served receipt without consulting `/healthz`,
then "ask and you will be told" is false of the world and this limit becomes a wire change rather
than a paragraph.

> 🔴 **That condition has been met, and this is the record of it firing** (`req/566` G-2, confirmed
> independently in `req/578` §5, ruling `req/38` §350 item 4). The consumer is this repository's
> own: `sdk/typescript/src/client.ts`'s `getReceipt` goes straight to `/receipts/{tid}` and calls
> `healthz()` nowhere, and the shipped suite that exercises it
> (`sdk/typescript/test/audit_m9_p4_tamper_and_errors.test.mjs`) reads a served receipt through it
> without asking. The falsifier names "an SDK sample" among the consumers it would count, and an
> SDK the project publishes is not a narrower thing than a sample of one.
>
> What this changes today is the status of the paragraph above and nothing else: by its own terms
> the limit is now owed **a wire change** — health that a receipt reader receives in band rather
> than has to know to ask for — and designing that is a separate lane's, filed rather than sketched
> here. Written down at the moment of firing, because a falsifier that is recorded in advance and
> then quietly not honoured is worse than one that was never written.

**🔴 And the wire change has been made.** `GET /receipts/{tid}` now answers a fourth member,
`server_health`, beside the document: `{"status": "ok" | "degraded" | "unhealthy" | "unknown",
"status_reason": string | null}`. It is **always present** — an absent key would mean both "this
server is well" and "this build does not carry the member", which is the same failure being
repaired, one step further along. `"unhealthy"` is the state `/healthz` answers `500
LEDGER_DISAGREES` to, so the reader who never asks now learns exactly what the reader who asks
learns; `"unknown"` is kept apart from it, because *the two files disagree* is a finding and *I
could not look* is the absence of one.

The three paragraphs above are left exactly as they were written, and this one is added under them,
because what they record is a decision that was right on the evidence it had and a condition that
was named before it was met. Two things about the shape are worth reading back. First, the
escalation those paragraphs predicted was **a header**, and this is a body member instead: a header
is invisible to precisely the reader it would be for — the SDK hands the decoded body to its caller,
and a consumer who does not know to ask `/healthz` does not know to read a header either. Second,
the limit itself is **not repealed by this**. The receipt is still served on a project whose two
files disagree, still unchanged, and still worth verifying offline against the key you pinned; a
signed statement about the past does not become false because a later frame was cut. What has
changed is that the document no longer travels without the deployment's own account of itself.

🔴 **What this still does not do.** It does not grade the receipt: there is no `verified`, no
`refuted`, no `inclusion` on it, for the reason a server that graded its own receipts would be
marking its own paper. It is not signed and cannot be — the signature covers DSSE's pre-authentication
encoding over `payload_type` and `payload`, which are minted before this layer sees them — so a
reader who does not trust this deployment should read it as a claim and not as evidence, and reach
for `gx receipt verify --offline`, which is unaffected by any of it. And it is bounded by the same
250 ms staleness window `/healthz` carries, plus that window's own invalidations. Finally, the wire
carries the answer but this repository's TypeScript SDK does not yet *read* it: the consumer whose
behaviour fired the falsifier is still the consumer that ignores the repair, and that is a separate
lane's, filed rather than papered over here.

**The two faces do not answer alike, and that is not repaired here.** `GET /candidates/{id}` catches
the engine up before it answers, and the CLI's read road asks whether the two files agree first. On
a merely-cut project the catch-up succeeds and the HTTP face answers 200 where `gx log proof` on the
same project refuses `LEDGER_DISAGREES`. The second probe in that file drives it, so the sentence
stays true of the tree rather than becoming an understatement.

**A server will not *start* here.** `gx serve` on a project whose files already disagree refuses at
start-up — *"a server that started here would sign checkpoints over a tree its own journal
contradicts"* — so this limit is about a project that broke while something was serving it, which is
the only way it is reachable and also the way it happens.

## A journal that is there and will not open is refused with `INTERNAL`, which is a generic and not a name (2026-08-22, R40, `req/553` M-01, `req/38` §328 ruling 2 ③/④)

**What R40 closed.** Make a project's journal unreadable — `chmod 0000`, or replace it with a
directory of the same name — and `gx log proof`, `gx log consistency` and `gx log checkpoint` used to
answer **exit 0**, `checkpoint` with a **signature**, on a project the same binary had refused
`LEDGER_DISAGREES` one second earlier. The read road treated "this build cannot open an engine" as
"there is no second file". It now asks whether the journal is **there** — `NotFound` and nothing
else — and refuses in every other case, including the case where the operating system would not let
it look. `crates/gx-cli/tests/r40_journal_presence.rs` has **eight** probes and drives all three
shapes with their negative controls.

**What is still owed.** The refusal for a journal that is a **regular file this process cannot
open** wears `gx_code` **`INTERNAL`**, and 44 §2.3 keeps that code for what *cannot be classified*.
This is classified: the operating system named the path and the reason, and `gx repair --json` prints
both under `engine_open_failed`. The vocabulary simply has no word for it — every one of the
seventeen rows was checked against this condition and each is false of it, `JOURNAL_ABSENT` ("is not
there") and `LAYOUT_BLOCKED` ("is not what the declaration says") most plainly, because the file is
there and is exactly what was declared. Minting the word means adding a row to spec 44 §2.3, which
is a decision above a repair lane, so it is filed as a DR and `INTERNAL` stands in the meantime — a
generic rather than a falsehood. The **journal replaced by a directory** does have an honest word and
wears it: `LAYOUT_BLOCKED`, whose title R40 widened from "a declared directory" to "a declared path"
so that a declared **file** with the wrong thing at its path can wear it truthfully.

**🔴 Update (2026-08-22, DR-B, `req/38` §337, `req/565` §3) — the DR minted `JOURNAL_UNREADABLE`.**
The paragraph above is left as written, because it was true through R41 and a reader comparing
this file with an older release should be able to see what changed and why. What changed: the DR
`req/38` §328 ruling 2 ④ filed against this exact condition was ruled — mint rather than let
`INTERNAL` stand. A journal that is present, is the regular file `req/56` §2 declares, and this
process could not open now wears `gx_code` **`JOURNAL_UNREADABLE`** (HTTP 500, CLI exit 1 — no new
exit number, `req/38` §148), not `INTERNAL`. `crates/gx-cli/tests/r40_journal_presence.rs::c8`
pins the new word in place of the old one. The vocabulary is now eighteen rows on the CLI face
(seventeen plus this one) and twenty-six on the wire (`sdk/typescript/src/errors.ts`).

**And a `stat` that failed is no longer reported as an absence.** `gx repair --json` printed
`journal_absent: true` about a journal holding 1,798 bytes whenever `.gx/ledger/` itself was
unreadable, because `Path::exists()` folds every error to `false`. An operator reading that would
restore from a backup over a file that was never lost. Only `NotFound` answers `true` now. 🔴 The
same fold is still present at **twenty-two** other `Path::exists()` and `Path::is_file()` call
sites in `crates/gx-cli/src/` that are not about the journal. R40 converted the journal family
only; the rest is filed rather than silently left.

> 🔴 **A correction to the two sentences above, and where their number comes from.** They were
> published saying **twenty-eight**, and naming the key store as an example — "a key that exists but
> cannot be `stat`-ed is reported as `NOT_FOUND`". Neither holds now, and one of them never did.
>
> The **example** was repaired: R41 converted the key store, the ledger's read door, the replay
> door and the verdict chain's reader, and a later round converted two more in `gx repair`'s own
> report. A published limit whose illustration has been fixed is a limit that reads as worse than
> the tree, which is the mirror of the failure this document exists to prevent.
>
> The **number** was wrong on the day it was written, by twenty-two per cent. It was taken from one
> column of a census — the `Path::exists()` column, thirty-seven sites, less the nine the journal
> family converted — while the sentence around it named `Path::is_file()` as well, and that
> column's ten sites were never in the arithmetic. Re-derived twice, because the round that found
> the error also moved the number: **twenty-three** on the tree the correction was written against
> (twenty-one raw `Path::exists()` and two raw `Path::is_file()`), and **twenty-two** on the tree
> this file ships with, because one of the twenty-three — `gx repair`'s chain key on the road where
> the engine opened — was converted in the same round. The twenty-two, by file: `declaration.rs`
> ×6, `demo.rs` ×1, `index.rs` ×1, `layout.rs` ×5, `main.rs` ×4, `repair.rs` ×1, `serve.rs` ×4.
> The count is of *raw* calls: sites reading a `FileType` a `stat` already returned, and mentions
> inside comments, are not folds and are not counted.
>
> 🔴 **And three more the count does not reach, in either direction.** `attach.rs`'s walk of `.gx/`,
> `layout.rs`'s check that every declared directory is one, and `gx repair`'s per-directory row all
> spell the same question as `symlink_metadata(..)` followed by `is_dir()`, which is neither
> `Path::exists()` nor `Path::is_file()` and so has never appeared in this number. R43 converted
> the second of the three — the door every verb is refused by first, which had been passing rows it
> could not look at. The other two are left standing and named here rather than left to be found: a
> walk that cannot `stat` a child does not descend into it, and a repair row it cannot `stat` is
> omitted. Neither states a falsehood; both are silent about a path they could not examine, and the
> answer for both is a wire that has somewhere to say so.

> 🔴 **Update (2026-08-23, R45, `req/621` M-1/L-1/L-3, ruling `req/38` §394) — the twenty-two's
> `repair.rs` ×1 is now zero, and three consequences this document had not written down.**
>
> **The last of the twenty-two.** The census two updates above named `repair.rs` ×1 as one of the
> twenty-two raw `Path::exists()`/`Path::is_file()` sites outside the journal family, and one round
> after that census was written it converted the ledger's own chain key beside it (`journal_absent`'s
> road, ¶ above). The last one stood at `repair.rs`'s **other** function — `report_without_engine`'s
> `ledger_present`, `.is_file()`, unconverted since R41 first named the fold — and audit 42 (`req/621`
> M-1) found it: a declared path holding a dangling symbolic link answered `false`, four lines above
> its own sibling `verdict_chain_present`, which already answers the R43 rule (`attach.rs::present`)
> correctly for the identical shape. Converted the same way: `Present(_) => Bool(true)`. **What this
> moves**: `ledger_present`'s meaning, in `gx repair --json`'s report, from "a regular file is at
> `.gx/ledger/journal.ledger`" to "something is at `.gx/ledger/journal.ledger`" — the same meaning
> `verdict_chain_present` and every other presence key in this report already carry. A monitor
> reading `ledger_present: true` learns that the path is not a genuine absence; it does not learn
> that the path is a regular file, and never could from this key alone (a directory or a broken link
> both answer `Present`, and `repair --json`'s per-directory rows are where a wrong shape is named).
> Audit 42 asked whether this shift lets a destructive end-state present as success — two attacks
> (`gx repair --yes` on the linked ledger, and the same link across an unmounted volume through
> `repair --yes` / `repair` / `submit`) did not produce one before this fix and were re-run after it;
> `req/643` carries the re-run.
>
> 🔴 **Correction (2026-08-25, DR-46-21 merge, SS631 item 3 / `req/741`): `req/643` never landed.**
> The number was reserved for this lane's own completion report, but the agent writing it was
> mistakenly killed before the file was created (`req/38` §405) — `git log --all` shows zero
> commits adding `req/643`. The independent verification this sentence promises landed instead as
> `req/650` (plus a second, fully-blind re-measurement, "650b"), which carries the re-run in full
> (`req/741` derives this mapping read-only and ground-truths it against `req/38` §399-§409). The
> sentence above is kept as the historical pointer written before the mistaken kill (no-delete);
> `req/650` is the citation to use.
>
> **A boundary this file had only given in counts, not in consequence (L-1).** `09_beds_v2.log`
> (audit 42) found four beds that satisfy DR-B's own predicate — declared, the shape the declaration
> says, present, and will not open — and answer `INTERNAL` rather than `JOURNAL_UNREADABLE`:
> `.gx/ledger/` itself unreadable, `journal.verdicts` unreadable, `journal.ledger` unreadable, and
> `.gx/VERSION` unreadable (this last one is already in this file, two updates above — "read
> …/.gx/VERSION: Permission denied" carrying `"gx_code":"INTERNAL"` in the same log line 591 read for
> a different reason). **This is not a fourth silence**: `req/38` §337's DR minted `JOURNAL_UNREADABLE`
> for exactly one path, `.gx/ledger/journal`, because that is the file R56 §2 names as this project's
> history and the one condition `req/227` M-04 and `req/229` M-02 had already measured an asymmetry
> against. Widening the word to the other four beds is a spec 44 §2.3 addition — a fifth row, a
> census gate to keep it whole (`r21_refusal_map_is_whole.rs`'s kind), a DR to file it — and R45
> (`req/38` §394) ruled **not now**: the buyer-facing cost of `INTERNAL` here is a generic rather than
> a wrong word, no attack surface changes it into a false positive the way a folded `false` did, and
> the four beds are rare shapes (a directory or a file made unreadable underneath a project already
> mid-repair) next to the ordinary loss DR-B was minted for. `INTERNAL` stands at all four, on
> purpose, revisitable at the R46 band if a buyer's read of `docs/LIMITS.md` says otherwise.
>
> **Two titles that said "is not there" about a path holding something (L-3).** `CONFIG_ABSENT` and
> `DECLARATION_ABSENT` are refusals this project raises when `.gx/config.toml` or `.gx/VERSION` is
> missing from a project that has a journal (`req/238` H-01, above). Their titles read "`…` is not
> there" — true of a genuine absence, and false of the shape `09_beds_v2.log` C7/C8/C9 constructed: a
> symbolic link at that path pointing at nothing. `attach.rs::present`'s rule, already the standard
> this file's `LAYOUT_BLOCKED` widening (¶ above, R40) and `JOURNAL_UNREADABLE` (¶¶ above, DR-B) both
> answer to, says a declared path holding a link is something that **is** there. Widened rather than
> branched, matching `LAYOUT_BLOCKED`'s own precedent: both titles now read "`…` is not there, or is
> a link that does not resolve", on **both faces** — `gx_cli::REFUSAL_MAP`'s rows and
> `gx_api::gx_code`'s wire constants moved together, because `wire_census.rs`'s F-2 gate (`req/38`
> §337) is the rule that a code's title is one sentence on both faces and R45 did not want to be the
> round that broke it silently while repairing a different one. No new `gx_code`, no exit change, no
> spec 44 row — the fact classified (something is at the path and this process cannot use it) was
> already `CONFIG_ABSENT`/`DECLARATION_ABSENT`'s fact; only the sentence was narrower than the fact.
>
> **The narrower fix this box needed, found by the suite that already existed.** The first shape of
> this fix asked `symlink_metadata` (`lstat`) at the top of `open_read_only_or_absent`, on the
> theory that only `NotFound` should read as absent. `lstat` does not follow the **final**
> component, so a merely dangling link and a self-referential loop both answered `Ok` under it and
> both fell through to the real open — which turned R43's own, *deliberately* lenient case (a
> dangling `journal.verdicts` on the healthy road, which must still let the engine open;
> `r43_presence_and_head.rs::s7_a_symlinked_chain_on_the_healthy_road_is_not_called_absent`) into a
> refusal. Caught by that suite before this fix shipped, and repaired to ask the narrower question:
> attempt the same open the writer's door attempts, and read only a `NotFound` from **that**
> attempt as absent. A dangling link still opens read-only and reads as an empty chain, unchanged;
> a loop, and everything else that is not `NotFound`, now falls through to the real error, matching
> the writer's door.
>
> **Unrepaired in the same round, named rather than left to be found.** `gx_log::store::LedgerStore
> ::open_read_only_or_absent` folds every `stat` failure into "absent" the same way
> `VerdictCheckpointStore::open_read_only_or_absent` did before this update — `Path::exists()`, same
> call, same crate, twelve lines above the one R45 converted (`req/621` L-2 named `journal.verdicts`
> only; the ledger's own reader's door carries the identical shape and was not in the box). A
> symbolic-link loop at `.gx/ledger/journal.ledger` should be expected to reproduce the same
> read/write asymmetry C4 measured, unverified by this round.
>
> 🔴 **Update (2026-08-23, R44 lane B item 2, `req/591` §4, `req/38` §369) — `attach.rs`'s walk now
> has that wire.** The paragraph above is left as written for the same reason the DR-B update above
> it was: a reader comparing this file with an older release should see what changed. What changed:
> `gx attach`'s JSON answer carries a fourth set, `unreadable_entries`, naming every directory under
> `.gx/` the walk could not list, project-relative. A subtree it cannot descend into no longer
> vanishes from both sides of the "before" and "after" diff without a trace — `created_entries` still
> cannot say what is inside an unreadable directory (the walk never reaches it), but
> `unreadable_entries` now says which directory that silence is about. `crates/gx-cli/tests/`
> `p1a_attach_placement.rs`'s `item2_unreadable_subtree_is_named_rather_than_silently_dropped` pins
> the shape live: a nested directory `chmod 000`'d after a first attach is named on the second, with
> the file inside it correctly absent from `created_entries` and explained rather than unexplained.
> One of the two remaining is now converted — `layout.rs`'s check was R43's — and `gx repair`'s
> per-directory row is the one still standing (`req/603` §4 filed it as reaching the same "to repair
> a project this process has to be able to open `.gx/` at all" wall R43 found, and left it a
> structural-reachability question for a later lane rather than a wire this one could close).

## An observation is evidence that a record was presented, never evidence that the operation happened (2026-08-26, `req/824` A1)

**The claim this qualifies.** That Glovrex governs deploys, env-var changes, config changes and log
windows on external platforms.

**What is measured.** That a registered attach-source presented a record, that the record was
schema-valid, that it chained to what we already held, and that it was committed to the ledger at a
stated sequence. `req/wire/fixtures/observation.jsonl` carries 19 vectors, of which 11 are refusals
or escalations (10 negative, 1 escalate — counted by the consuming test
`crates/gx-core/tests/observation_class.rs`, which drives the envset subset against the codec; the
drafted row in `req/wire/limits_rows.md` said 9 and the measured number is 11, corrected here per
`req/836`). `crates/gx-core/tests/observation_class.rs` has **seven** probes and lives in the
private tree only — its bed sits under `req/wire/`, which the published tree does not carry
(`req/839`).

**Why.** We never run the workload and we cannot read the platform (SS273, `req/733` §3). Everything
about the operation reaches us because somebody chose to report it.

**What holds in the meantime.** Coverage is attach-source-**declared** and is rendered as
numerator/denominator, never as "all". The registry surface itself lands with `req/824` A4;
`coverage_verified` is declared from A1 so that surface cannot land without carrying it, and it is
always `false` in this phase — the declaration cannot be mistaken for a measurement.

**What this does not fix, said in the same breath.** Operations that were never reported are invisible,
and their absence is indistinguishable from there being none. Nothing here detects a source that
under-reports.

## Glovrex never sees an environment variable's value, and the names it does see are in clear (2026-08-26, `req/824` A2)

**The claim this qualifies.** That env-var change history is governed.

**What is measured.** The ordered set of `(name, value_digest, scope)`, where `value_digest` must match
`blake3:<64 hex>` client-side-salted form. A value field of any other shape is refused at the gate
(`PLAINTEXT_SECRET_REFUSED`), with four adversarial vectors covering raw plaintext, wrong-length hex,
base64-of-plaintext, and empty.

**Why.** Holding values would make Glovrex a secrets store, which is a different product with a
different threat model. The refusal is a product ruling and not input validation.

**What holds in the meantime.** The salt is computed and held client-side and is never transmitted, so
we cannot reverse a digest even for values we could otherwise guess.

**What this does not fix, said in the same breath.** Two real gaps, stated rather than left to be
found. First, **names travel in clear** — deliberately, because the diff is meaningless without them —
so a variable name that is itself sensitive (`ACME_INTERNAL_PROJECT_KEY` reveals that the project
exists) is disclosed to anyone who can read the ledger. Second, a digest of a **low-entropy** value
protects little against a party who obtains the salt and enumerates candidates; the fixture bed accepts
that case rather than pretending the detector catches it.

## An observation cannot be undone at the substrate, and for log windows it cannot be undone at all (2026-08-26, `req/824` A1)

**The claim this qualifies.** Pillar 2 — the inverse-escrow undo guarantee.

**What is measured.** That the prior observed state is held in **record-level** escrow, and that undo
of an observation returns a typed refusal rather than a success.

**Why.** Substrate-level invertibility requires a substrate we can write to. For an attach-source there
is none and there never will be (SS273). For log windows the refusal is stronger and deliberate: undoing
an attestation would un-attest history.

**What holds in the meantime.** The two refusals are **typed** — `inverse-not-executable-at-substrate`
and `append-only-class`, `gx_engine::Error`'s own kinds, constructible engine-side only
(`crates/gx-canon/tests/authority_boundary.rs` counts constructions in the secondary surfaces and
holds them at 0; `crates/gx-canon/tests/authority_boundary.rs` has **nine** probes) — so the
absence is declared at the exact point a caller would otherwise assume a
capability. The one class where a real invert does hold is config in `adapter` mode, where the blob
lives in a repo the git/fs adapter covers and AC-050's bit-equal round-trip applies as it does for any
file; the `substrate: {adapter|declared}` field exists so the two are never conflated.

**What this does not fix, said in the same breath.** Both refusals currently fold onto the single
`INVERSE_UNAVAILABLE` code, together with the ordinary missing-escrow case. Three situations, one code;
only `detail` separates them. That fold is written down in `req/wire/gx_code_additions.json` and in
`gx_code.rs`'s two fold rows rather than repaired, because three codes for a distinction no caller acts
on differently would be vocabulary grown for its own sake.

## The registry records what a source declares it covers, and Glovrex does not verify the declaration (2026-08-26, `req/824` A4)

**The claim this qualifies.** That registered attach-sources are "covered" by Glovrex.

**What is measured.** That a source registered, what family it named (`vercel` / `github-actions` /
`generic-ci`), what it **declared** it reports, and — when it presented one — that its public key is
a decodable ed25519 key. `req/wire/fixtures/attach_source.jsonl` carries 10 vectors (5 positive,
5 negative), driven by `crates/gx-api/tests/attach_sources.rs`.
`crates/gx-api/tests/attach_sources.rs` has **four** probes and lives in the private tree only,
for `observation_class.rs`'s reason above (`req/839`).

**Why.** `declared_coverage` is the source's own claim; verifying it would require reading the
platform, which SS273 rules out permanently. `coverage_verified` is present on every registry
response and is always `false` in this phase, so the declaration cannot be mistaken for a
measurement.

**What holds in the meantime.** Every response carries a non-empty `limits` array (P-12), and a
source registered without a public key has "membrane Bearer only" stated in that array rather than
being silently presented as a signed one.

**What this does not fix, said in the same breath.** A registration does not survive a server
restart in this atom: the registry is membrane state behind the lock, not a `.gx/` file — a disk
home is deferred, with its reasoning, in `crates/gx-api/src/attach_sources.rs`'s declared delta 1.
And a registered key proves the source *can* sign; nothing signs or verifies against it until the
observation ingest route (`req/824` A5) exists to present signed observations.

## Platform-push ingestion does not exist; everything arrives because a client sent it (2026-08-26, `req/824` A5)

**The claim this qualifies.** That Glovrex observes platform operations.

**What is measured.** Client-push only — an authenticated attach-source POSTing
`/attach-sources/{id}/observations` over the existing Bearer + idempotency membrane, and the
result travelling the ordinary candidate → verify → commit road (no second pipeline; the response
is an ordinary candidate id). `crates/gx-api/tests/observations.rs` has **four** probes and lives
in the private tree only, for `observation_class.rs`'s reason above (`req/850`): it drives all 19
bed vectors (8 positive / 10 negative / 1 escalate), the four E2E class roads to a receipt, the
chain-gap Escalate road to `GET /escalations`, and the `observation_id` idempotency control.

**Why.** There is no webhook receiver. The only wire transport beside HTTP is
`gx-adapter-mcp/src/transport.rs`, which is MCP-protocol-only; `req/805` §2-2 verified the absence
rather than assuming it.

**What holds in the meantime.** The absence is rendered as typed absence (`req/805` P-03) rather
than as an empty feed, so no face shows a webhook stream that is silently never populated. A chain
gap is admitted into **Escalate** — the third state, a 2xx with `CHAIN_GAP_ESCALATE` and a ticket
reachable at `GET /escalations` — never a silent accept and never a Deny that would discard real
evidence. And undo of a committed observation is a **typed refusal** at the engine
(`inverse-not-executable-at-substrate` / `append-only-class`), produced now rather than declared:
the ingest route is these kinds' first live producer road.

**What this does not fix, said in the same breath.** Three declared holes. First, a source that
stops reporting produces the same picture as a source with nothing to report; closing that needs
the `req/805` Phase X sockets (U2/U3), design-frozen and not built here. Second, **no shipped
policy pack permits the observation substrate**: Cedar is default-deny and every shipped permit is
scoped to its own substrate, so a deployment that ingests observations composes its own
`custom:observation` permit into its policy set — the shipped-pack seat is a pack+adapter pair
decision (`req/38` §60) deliberately not taken by this atom. Third, the ingest road's chain heads
and replay map are in-memory: a restart keeps every candidate (they are in the journal) but
forgets the replay short-circuit and the chain-continuity memory, declared in the road's own
module header rather than silently decided.

## An observation is now published whole or not at all, and until this release it was not (2026-08-26, `req/859` G8, `req/868`)

**The claim this qualifies.** That a content-addressed name in this product is a digest of the
bytes filed under it.

**What is measured.** `ObservationStore::put` writes through `write_atomically` — temp file,
`sync_all`, `rename(2)`, parent-directory fsync — the same **one body** `BlobStore::put` uses.
Before this release it was `File::create` at the final path followed by `write_all`, so for the
width of that gap the content address held a file that was not the content. That is not a
theoretical window: `crates/gx-engine/tests/g8_observation_atomicity.rs` has **two** probes. The
first runs a writer beside a directory-listing observer and measured **485 partially-published `.obs`
files across 2,321,474 observations** against the old writer, and **0 across 2,653,294** against the
new one.

**The verification half, which the first landing missed** (`req/871` F4). Closing the write window
stops the product *creating* a truncated body at a content address; it does nothing for a tree that
already holds one. `put` answered `AlreadyPresent` on a bare `path.exists()` — trusting the **name**
where content addressing is a promise about the **bytes** — so such a tree could never heal. It now
re-reads and byte-compares, exactly as `BlobStore::put` has since R9, and republishes on a mismatch.

**Why it mattered more than it looked.** The read side already failed closed — `get` re-hashes and
refuses a body that does not match its name — but it failed *quietly*: a truncated observation
makes escrowed-inverse completion fold to `Unavailable`, which tells an operator the observation is
**gone** when the truth is that it was **half-written**. This is the same shape `req/236` H-01
measured and R9 repaired, on the blob store only; the repair lived as a private method on one
store, so the second content-addressed store in the same file never got it.

**What this does not fix, said in the same breath.** A crash still loses the observation — it just
loses it under a name no reader resolves (`<cid>.obs.tmp.<pid>`) instead of publishing a lie at the
content address. Cleanup is not the defence and never was: a power cut does not run `remove_file`,
so the residue is `gx repair`'s to report. And the test buys its assurance from a **concurrent
reader**, not from a real crash — a `#[test]` cannot cut power, so what is measured is the window's
existence and closure, not the kernel's behaviour across a reset.

## A file's bytes are fsynced on every platform; its *name* is only on unix, and this repo's own working tree is not unix (2026-08-26, `req/859` G9, `req/868`)

**The claim this qualifies.** That a committed transformation survives a crash.

**What is measured.** On unix, `sync_parent_directory` opens the parent directory and fsyncs it
after every publish, so the directory entry is as durable as the bytes. Measured platform is
x86_64 Linux only (`req/52` §5, A-5); every other unix inherits the *call*, not the *measurement*.
Off unix the function is `Ok(())`: file contents are still `sync_all`'d, but the directory entry
naming them is not, so a crash can lose the **name** of a file whose bytes reached the device.

**Why the Windows path is not implemented.** Because the honest Windows answer is not a translation
of `fsync(dirfd)`. `FlushFileBuffers` on a directory handle is not a supported operation, and the
usual claim put in its place — that NTFS journals metadata, so the entry is durable once the file's
own flush returns — is a property of a filesystem **we have never measured**. An `Ok(())` renamed
as a Windows implementation would be a guarantee we did not earn. The same store records the
neighbouring unmeasured fact: `.gx/LOCK` is not measured on Windows, on 9p (`/mnt/c`) or under a
file-sync client, and this repository's working tree is Windows under OneDrive.

**What holds in the meantime.** The gap stopped being a doc comment. `NAME_DURABILITY` is a typed,
exported constant selected by the *same* `cfg` that selects the implementation, carrying a sentence
fit to show an operator verbatim, and `crates/gx-engine/tests/g9_name_durability.rs` has **three**
probes: it fails if a later lane widens the "held" arm without bringing a measurement, or narrows the
implementation while the declaration still boasts.

**What this does not fix, said in the same breath.** It is a **declaration, not a warning**:
nothing in the workspace prints it yet, because gx-engine has no logging surface of its own and the
operator-facing half — a line at `gx serve` start, or a `/healthz` member — is a CLI/wire change
this lane did not have the box to land. Until that lands, an operator running on Windows is not
*told*; the fact is merely available to be asked for. `req/868` carries the residual.

## `gx-adapter-fs`'s own directory fsync had the same un-`cfg`-gated gap as the journal's, and reported apply failure for a rename that had already landed (2026-08-26 `req/868` R-868-5, closed 2026-08-29 `req/919` W4)

**The claim this qualifies.** The item directly above: that a file's bytes are fsynced on every
platform but the directory entry naming them is only fsynced on unix.

**What was measured wrong.** `req/868` found the sibling of G9 in the adapter that actually writes
an `apply`: `crates/gx-adapter-fs/src/apply.rs` opened and `sync_all`'d the target's parent directory
**un-`cfg`-gated**, at both the call site after a whole-file write and the call site after a removal.
On native Windows, opening a directory with `std::fs::File::open` and syncing it fails --
`FlushFileBuffers` is not a supported operation on a directory handle there either, the same fact G9
measured for the journal -- so `apply` reported `ApplyFailed` for a rename or removal that had
**already landed on disk**. The report and the world disagreed in the harmful direction: an operator
reading the error would retry or escalate a change that did not need it.

**What is fixed.** Both call sites now go through one `#[cfg(unix)]` / `#[cfg(not(unix))]` function,
`sync_parent_directory`, so a landed change is never reported as a failure off unix. The gap this
closes is only the false-failure report; it does not add a Windows directory-fsync implementation
that does not exist (none does, for the same reason G9 gives above).

**What holds in the meantime, and what does not.** This crate carries its own typed declaration --
`gx_adapter_fs::NameDurability` / `NAME_DURABILITY`, the same shape as `gx_engine::NAME_DURABILITY`
-- rather than depending on `gx-engine` for it: `gx-adapter-fs` has **zero** workspace dependencies
beyond `gx-core`/`gx-canon`/`gx-substrate` (crate root, **E-M4-26**), and an adapter importing the
engine that consumes it would invert the layer direction. The two declarations answer the same
platform fact for two different directories (the engine's journal, this adapter's target parent) and
are duplicated by design. As with G9's copy, this is a **declaration, not a warning**: the
operator-facing half (a line an adapter caller could show) is not wired here, because printing it is
a CLI/wire change out of `req/919` W4's box (AC: the `cfg`-gate and this doc sync only). `req/868`'s
own **Residual R-868-1** for the engine's copy applies unchanged to this one.

**What this does not fix.** It does not make Windows directory-entry durability held; it makes the
*absence* of that guarantee stop masquerading as a failed write. `crates/gx-adapter-fs/tests/apply_durability.rs`'s
ordering probe was updated in the same lane to look for the new call name (`sync_parent_directory`)
in place of the literal `sync_all()` it used to find at that position; the **order** it asserts --
temp-file fsync, then rename, then the directory-durability step -- is unchanged, and the change is
recorded in the test's own comment with the ruling it cites (`R-868-5`), per the discipline that a
test may only be edited when a named, dated ruling says the behaviour it pinned was a defect.
