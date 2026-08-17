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

8. the Lean model under `lean/` proves eight theorems and five counterexamples about
   the F0 specification (kernel-checked, no `sorry`); the Rust implementation is
   compared against that model by a differential test -- 1,500 conformance vectors,
   six kinds, on every push -- and a comparison is a difference check, not a proof: no
   refinement theorem connects the two. of the checks that run automatically on every
   push today, 16 of this project's 17 workspace crates are covered (`probes/doubt`
   runs by hand because its subject lives outside the repository) and the TypeScript
   SDK's own tests are not run by CI at all -- their green is a person's run, not a
   machine's.
   (45 §4.2 (v0.2.3 note; v0.4-l present-tense note); 51 §11.1 (v0.2.3 note; v0.4-l present-tense note))

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

---

`docs/TUTORIAL.md` ends on this page for the same reason it opens the list above: a
walkthrough that stopped at the parts that work would be a walkthrough that let you find
the parts that do not on your own, later, on something that mattered.
