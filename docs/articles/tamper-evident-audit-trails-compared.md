# Four kinds of record, one tampered byte

A plain append log, a rotating log, a git history, and a signed receipt were each given the same
two tests: change one byte inside them, and then ask what they can still answer about last month.
Two of the four noticed the byte. One of those two stops noticing the moment the change is made
by someone holding normal write access rather than by a failing disk. This page is the
measurement, including the test our own receipts fail.

## The falsifier, first

The claim being tested is ours: *a receipt lets someone who is not the producer detect that a
record changed, offline, and does not expire on a timer.* Three results would refute it.

1. Flip a byte in a receipt and have the verifier answer `0`. It answers `7`.
2. Find a retention sweep that deletes receipts on a clock. There is no deletion path in the
   published crates at all, and the one retention constant is a floor rather than a ceiling.
3. Produce a receipt that verified once and later stopped verifying against a build from the
   same project. **This one fires.** A receipt this project issued on 18 August 2026 returns
   exit `7` against the current build. The details are in the last section, and they are not
   flattering.

A comparison written by the people who ship one of the things being compared is worth exactly as
much as its worst-looking row. That row is number 3, and it is why it appears above the results
instead of below them.

## What each design is for

Before any of this becomes a table, the four designs answer different questions on purpose, and
three of them are answering their own question correctly.

A **plain append log** is the cheapest durable record that exists. One open file, one write call,
readable by every tool anyone already has. It is the default for good reasons, and any argument
that starts by treating it as naive has skipped the part where it is universal and free.

A **rotating log** exists because an unbounded log is an operational hazard. A disk that fills
because a debug record grew without limit takes the service down with it, and the service is the
thing users came for. Rotation is a deliberate trade of history for a bound, and an operator who
sets `rotate 2` is not being careless. They are choosing which failure they would rather have.

A **git history** is a hash chain over content, built to answer how a tree reached its current
state and to make accidental corruption loud. It was designed against disk faults and transport
errors by someone who assumed both.

A **signed receipt** is narrower than all three. It answers one question, about one change, for
someone who was not there and does not trust the producer. It is a bad choice for debugging and a
worse one for reading by eye.

Agent runtimes generally build the first three shapes rather than the fourth, and their own
documentation is usually clear about it. The section below quotes one that is unusually clear.

## Test one: change one byte

Every tamper below is the same shape. One byte, edited in place, file size unchanged, so nothing
is detectable from a directory listing.

### Plain append log

Five JSON records, one per line, describing writes to a file. The third record said `"bytes":30`.
One byte moved it to `"bytes":39`.

```
cmp -l intact.log tampered.log
289  60  71
```

Exactly one byte differs, and the file is the same 485 bytes as before. Then the commands an
operator actually runs:

| after the edit | result |
|---|---|
| `wc -l` | 5, unchanged |
| JSON parse of every line | 5 records parsed, 0 errors |
| `grep -c write_file` | exit 0, unchanged |
| append a new record, parse again | 6 records parsed, 0 errors |

The tampered file is a completely valid log. It parses, it greps, it keeps accepting writes, and
it says something different from what happened. Nothing in the file or the tooling is capable of
objecting, because nothing was ever computed that the edit could contradict. This is not a defect
in append logs. It is the whole shape: an append log records, and recording is not attesting.

### git history

Three commits, then the stored object for the middle commit was corrupted by one byte.

```
git fsck
error: inflate: data stream error (invalid literal/lengths set)
error: unable to unpack header of .git/objects/33/49aa4...
error: 3349aa4...: object corrupt or missing
exit 3
```

git caught it. `git log` refused to run at all, exiting 128. Of the four designs, git is the one
that does the thing people assume all of them do, and it does it without being asked or
configured. Any comparison that leaves this out is selling something.

Then the same repository was tampered with a second way. Instead of corrupting an object, the
history was rewritten the way a person with write access rewrites it:

```
git commit --amend      # change 3 now contains different content
git reflog expire --expire=now --all
git gc --prune=now
git fsck                # exit 0
```

| after the rewrite | result |
|---|---|
| `git fsck` | exit 0, no output |
| `git log --oneline` | three commits, same three messages |
| `git rev-list --count HEAD` | 3 |
| the previous commit object | gone, `git cat-file -e` exit 1 |
| `git reflog` | 0 entries |
| the file's contents | changed |

The rewritten history is internally perfect. Every hash checks, the chain is intact, the commit
messages read the same, and the content is different. This is correct behavior and not a bug: git
is a version control system, and rewriting history is a feature it is supposed to have. The hash
chain proves internal consistency, which is a different claim from proving that this history is
the history that happened. A hash chain binds a record to itself. It does not, by itself, bind
that record to a moment when someone else could see it.

### Signed receipt

Same shape of edit, on a receipt produced by the walk in the project tutorial. One byte inside the
DSSE payload, in place, file size unchanged at 1306 bytes.

```
cmp -l receipt.orig.json receipt.json
126  117  101
```

The tampered receipt is still valid JSON and still parses. Then:

```
gx receipt verify $RECEIPT --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

| receipt state | exit | answer |
|---|---|---|
| intact | 0 | `valid:true`, signature true, inclusion verified |
| one byte flipped in the payload | 7 | `valid:false`, refusal: `no valid signature under key ...` |
| checkpoint's `root_hash` edited instead | 7 | `anchor_authenticated:false` |
| verified against a different key | 7 | `valid:false`, signature false |

Exit `7` means refuted. The receipt does not merely fail to parse, and it does not merely warn.
The verifier reaches a verdict about the document and reports which check produced it. That last
part matters more than the number: a tool that refuses without saying which check failed is
asking to be trusted, which is the thing this design is trying to avoid needing.

The command has no `--project` in it. It reads three files, and it was run on receipts a stranger
could have been handed.

## Test two: what can it answer about last month

Tamper-evidence is worthless if the record is gone. This is the axis that gets left out of
comparisons, and it is the one where the honest answers are least flattering to everyone.

A rotating log was configured with `rotate 2` and cycled five generations, using logrotate 3.21.0.

| question | answer |
|---|---|
| files remaining | `agent.log`, `agent.log.1`, `agent.log.2` |
| contents | generations 5 and 4 |
| is generation 1 anywhere on disk | no, `grep -rl` exit 1 |
| does the state file record that generation 1 existed | no, 0 mentions |
| does anything at all record the loss | no |

Generation 1 did not just expire. It left no trace of having expired. A reader of the surviving
files cannot distinguish "there were only ever three generations" from "there were fifty and you
are looking at the last three." The bound was chosen deliberately and the deletion is correct. The
gap is that the deletion is silent, and silence about a deletion is indistinguishable from
absence.

This is worth stating carefully because it is the single most common shape in production. Bounded
retention is the norm, and the norm is fine right up until someone asks a question about a window
that has closed.

### One agent runtime's stated bounds

OpenClaw is a large, widely deployed agent runtime, and it documents this boundary better than
most projects document anything. It keeps three separate record planes, and the retention on each
is written into the source as a constant:

| plane | bound (read from source) |
|---|---|
| trajectory runtime events | 14 days max age, 512 MB global, 10 MB per session capture |
| audit ledger | 30 days, 100,000 rows |
| session state events | 30 days, 50,000 rows |

Every one of those is a deliberate, documented bound rather than an oversight, and their own
documentation is the clearest statement of what the records are for. Their audit page says the
ledger is "bounded, metadata-only" and that queries "never return records older than 30 days."
Their trajectory page calls that plane a "per-session flight recorder" and says bundles are "for
support and debugging, not public posting."

Their limits section is better than most vendors' and says the quiet part directly:

> Treat it as evidence of what was recorded, not as proof of what happened.
> Absence of a row proves nothing.
> This ledger supports debugging and operational review. It is not a lossless compliance archive;
> if you need one, use an external system fed by OpenTelemetry or channel-level tooling.

That is the correct description of a flight recorder, written by the people who built it. A flight
recorder is meant to be bounded. It is meant to be metadata. It is meant to be enough to debug
last Tuesday, and it explicitly tells you to go elsewhere for the other job.

One of those three planes also does the thing the rotating log above could not. Before deleting
old session state rows, it stamps a per-session watermark recording how far the deletion reached,
and a reader asking for older events gets an explicit `historyGap` computed from that watermark
rather than from arithmetic over missing sequence numbers. The source comment says the stamp
happens before the delete on purpose. That is a bounded window that can still tell you it is
bounded, which is strictly better than silence and is the detail most retention code skips.

Two further details from the source, because they matter for the export axis and not as criticism.
The audit plane pseudonymizes identifiers with an installation-local HMAC and fails closed if the
key material is corrupt, which is a stronger privacy posture than most audit trails bother with,
and their docs correctly describe it as "correlation, not anonymization" rather than overselling
it. And the trajectory exporter re-sorts merged events and assigns fresh sequence numbers at
export time, with the bundle manifest carrying byte counts rather than digests. That combination
means an exported bundle is a faithful support artifact and not a self-proving one, which is
exactly what a support artifact is supposed to be. They say so on the page.

So the honest summary of this system is not that it is missing something. It is that it built a
flight recorder, said it built a flight recorder, and pointed at the door for the other job. The
rest of this page is about what walking through that door actually costs.

## What the receipt design does about retention

The published tree was searched for a retention sweep, a prune path, or a max-age constant across
every crate. There are three matches. One is a 250 millisecond health snapshot cache, one is its
use, and the third is this:

```rust
pub const NFR_027_MINIMUM_RETENTION_DAYS: u32 = 180;
```

A minimum, not a maximum. The comment above it explains that the log crate exposes no public
function that edits or removes an entry, and a test scans the exported surface on every run to
assert that no such function exists. The floor holds because there is no deletion API to undercut
it.

This is a real difference in polarity, and it is also a cost rather than a free win. A store with
no deletion path grows without bound, and an operator who needs a bound has to build one outside
the crate, at which point they own the problem that rotation was invented to solve. Choosing a
floor over a ceiling moves the operational hazard rather than removing it. That trade is only
worth making for records whose entire purpose is to still exist when someone asks later, which is
why it would be a poor choice for a debugging plane and a reasonable one for a receipt.

## The same table, with our own row filled in

| | plain log | rotating log | git history | flight recorder plane | signed receipt |
|---|---|---|---|---|---|
| detects a byte edit | no | no | yes, exit 3 | not measured | yes, exit 7 |
| detects an authorized rewrite | no | no | no, exit 0 | not measured | yes, signature fails |
| records that data was deleted | n/a | no | n/a | partly, one plane stamps a prune watermark | no deletion path |
| third party can check without the producer | no | no | partly | no | yes, measured |
| exported artifact carries its own proof | no | no | yes, within the repo | no, byte counts not digests | yes |
| retention polarity | unbounded | ceiling | unbounded until rewritten | ceiling, 14 to 30 days | floor, 180 days |
| readable by tools you already have | yes | yes | yes | yes | no |
| useful for debugging last Tuesday | yes | yes | somewhat | yes, this is its job | poor |

Two cells in that table are inference rather than measurement, and should be read as such. The
rotating log's tamper rows are inherited from the plain log, because rotation moves files without
changing their format, so the plain log measurement covers both. The flight recorder column's
tamper rows are marked not measured because no OpenClaw process was run.

The last two rows are the ones we lose. A receipt is not readable by eye, needs a specific binary
to interpret, and is close to useless when the actual question is why a run behaved oddly. Anyone
whose real problem is debugging should reach for the flight recorder, and the flight recorder's
authors are right to tell them so.

## What the receipt design cannot do

These are from the project's own limits page, which runs to about 3,500 lines and comes before
the pitch on purpose. The ones that bear on this comparison:

**It only sees what goes through it.** An agent that starts its own server, or a call that only
reads, is outside the proxy by declared design. Effects that never cross the boundary produce no
receipt, and no receipt is not evidence that nothing happened. This is the same shape as "absence
of a row proves nothing," and quoting that line approvingly while pretending we escape it would be
dishonest.

**It cannot see through an adapter that lies underneath it.** If a server resolves one path to
another internally, the record names the address that was asked for and never learns what was
touched.

**Undo is rarer than it sounds.** Across a first census of public tools that write anything, a
real inverse existed for about 13.8% of them. For the rest there is no restoring call to send, and
one cannot be invented.

**Root writes around all of it.** An attacker with kernel privilege bypasses the whole mechanism,
and this build does not detect that.

**A verdict count proves only that nothing was hidden from the policy, not that the policy was
strict.** Weaken the rule to admit everything and the count still reads clean.

**And the one from the top of this page.** A receipt this project issued on 18 August 2026 does
not verify against the current build. Six probes hold those bytes and assert, on every run, that
the signature still checks out and the payload still does not decode. Two fields were added to the
receipt payload as required with no default, and either alone refuses the old file. The verifier
answers exit `7` about a document nobody touched, which means the number this page uses for
"someone edited this" is currently also the number for "we changed our own schema badly." The
project's limits page states this in its own words, calls it a mistake rather than a trade-off,
and notes that the same kind of change was made correctly twice afterward.

That is the honest state of the durability claim. The design holds a floor rather than a ceiling
and has no deletion path, which is the part that is true. The oldest receipts do not survive a
schema change in the issuer's own codebase, which is the part that is not yet true, and until it
is, "verifiable without the issuer" means without the issuer's server rather than without the
issuer's next release.

## Measured, read, and not compared

Every claim above falls into one of the buckets below, and mixing them would defeat the point.

**Run, this session, on this machine.** The plain log tamper and its parse results. The logrotate
cycle with logrotate 3.21.0. The git corruption test, the git rewrite test, and every exit code
from both. The full receipt walk on a fresh anonymous clone of the public repository, HEAD
`177141e`, built from source in 67 seconds on WSL2 Ubuntu 24.04 with a dev profile, including
verify exits 0 and 7, the three tamper variants, and the undo restoring the file. The retention
grep across the published crates.

**Read from source, not run.** Every OpenClaw number: the three retention constants, the byte
caps, the export-time sequence renumbering, and the manifest fields. Those come from reading the
code and its documentation at a clone of commit `e8a158e`. The gateway was not installed and no
OpenClaw process was started, so nothing here is an observation of its records being written or
pruned in a live system. If any constant has changed since that commit, the numbers move and the
shape of the argument does not.

**Documented, not run.** Every quoted sentence from the OpenClaw documentation is quoted because
it is their own description of their own design, and it is more precise than a paraphrase would
be.

**Not compared at all.** No hosted or commercial audit product appears here, because their claims
cannot be measured from outside and repeating a marketing table would be the opposite of this
exercise. Two other open source projects that would have belonged in this comparison are absent
by policy, because this project currently has pull requests open and unmerged against them, and
auditing a project while asking it for something is not a thing to do. This is therefore a
measurement of four designs, not a survey of a field.

One more thing found while measuring, since it is a defect in our own documentation rather than
anyone else's: the tutorial's third line points readers at a `gx demo` subcommand that the public
binary does not ship. It exits 1. The walk in section 2 of that page is the one that runs.

## Reproducing this

The receipt half is the tutorial walk in `docs/TUTORIAL.md`, section 2, on a fresh clone. Nothing
in it was modified for this page. The conventional half needs no special tooling: five lines of
JSON, `dd` with `conv=notrunc` to move one byte without changing the length, `cmp -l` to prove
exactly one byte moved, and then whichever reader you would actually have used. The git half is
three commits, one corrupted object, and then `--amend` with the reflog expired.

The result that should be checked first is the one that would end the argument. If a byte can be
changed inside a receipt and the verifier still answers 0, none of the rest of this matters.
