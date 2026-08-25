<div align="center">

<img src="https://github.com/TraceFold/tracefold/releases/download/brand-assets/banner.png" alt="Tracefold — the inverse is sealed before the action" width="880">

### Undo, but proven.

**Tracefold** is verified undo and offline-verifiable receipts for AI agent actions, in Rust.
An agent's change is held with a checked inverse **before** it lands, and every verdict
becomes a receipt anyone can verify offline — without trusting whoever issued it.

The inverse is checked and held **before** the effect; after a rollback, `gx` **verifies the
world actually went back** before saying so.

<p>
<a href="#what-this-does-not-cover"><img alt="the limits first" src="https://img.shields.io/badge/the%20limits%20first-0b0a09?style=for-the-badge&labelColor=0b0a09"></a>
<a href="https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md"><img alt="technical report" src="https://img.shields.io/badge/technical%20report-0b0a09?style=for-the-badge&labelColor=0b0a09"></a>
<a href="https://discord.gg/bFBvvg7AG"><img alt="discord" src="https://img.shields.io/badge/discord-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=discord&logoColor=ece7da"></a>
</p>

<p>
<a href="https://github.com/TraceFold/tracefold/blob/main/LICENSE"><img alt="license" src="https://img.shields.io/github/license/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
<a href="https://github.com/TraceFold/tracefold/commits/main"><img alt="last commit" src="https://img.shields.io/github/last-commit/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
<a href="https://github.com/TraceFold/tracefold"><img alt="language" src="https://img.shields.io/github/languages/top/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
</p>

</div>

[Build](https://github.com/TraceFold/tracefold#build-it) ·
[Verify a receipt](https://github.com/TraceFold/tracefold#verify-a-receipt-without-us) ·
[Limits](https://github.com/TraceFold/tracefold#what-this-does-not-cover) ·
[Where it stands](https://github.com/TraceFold/tracefold#where-it-stands) ·
[What it does](https://github.com/TraceFold/tracefold#what-it-does) ·
[Report](https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md) ·
[Contributing](https://github.com/TraceFold/tracefold/blob/main/.github/CONTRIBUTING.md) ·
[Discord](https://discord.gg/bFBvvg7AG) ·
[Glovrex](https://glovrex.com)

---

> [!IMPORTANT]
> **Not released.** The names `tracefold` on npm and on crates.io are ours and are taken,
> but what sits behind both is an empty 0.0.1 placeholder holding the name, published on
> 12 and 13 August 2026. Installing it gets you nothing. What you can actually run today is
> a build from source, and nothing below should be read otherwise. The download counters on
> those pages are mirrors and scanners fetching a new name once: 110 of the 125 npm
> downloads landed on the day of publication, and the last two days are zero.
>
> **Who this is for.** Someone who will later have to show a third party what an agent
> did, and to whom "we checked our logs" is not an acceptable answer: an audit, a customer
> contract, a regulator, an internal review. If you only want to undo a mistake, the
> service you are already using probably keeps enough history, and you do not need this.
>
> **Two things your agent's own client cannot do for you, however good it gets.** It cannot
> see what left it: a rewind feature covers the conversation and the edits it made itself,
> not what went out through a shell command or through someone else's tool server. And it
> cannot be the independent check on its own work, because the party being audited grading
> its own paper is not an audit, at any level of accuracy. Both are questions of position,
> not of features, so they do not close when the client improves.
>
> **Implemented is a different word.** The engine, the ledger, the policy gate, three
> substrate adapters, the MCP wrapper, the CLI, the HTTP API, an SDK and a machine-checked
> model are written and tested. Released is the one that is not yet true.

## Build it

```sh
git clone https://github.com/TraceFold/tracefold
cd tracefold
cargo build --workspace
```

The binary is `gx`. Rust stable; the toolchain is pinned in `rust-toolchain.toml`. The
subcommands that exist today: `submit`, `plan`, `verify`, `commit`, `undo`, `cancel`,
`escalation`, `receipt`, `replay`, `log`, `key`, `policy`, `serve`.

**Free forever for one person.** Receipt generation, offline verification and self-hosting
are unlimited and unexpiring for a single person using this alone — a promise, not a price
(the one exception to "no pricing on this face," a standing ruling dated 21 August 2026).

## Verify a receipt without us

This is the part that needs no build, no account, and no trust in whoever issued the
receipt. Given the binary and three files — the receipt, a checkpoint, a public key —
anyone can re-check the claim offline:

```sh
gx receipt verify <receipt> --offline --checkpoint <checkpoint> --key <public key>
```

It exits `0` when the receipt holds and `7` when it does not. No workspace is read, no key
store is opened, and no network call is made. A verifier that phoned home would be asking
you to trust the thing it is meant to be checking.

## Figures you can re-derive

Numbers about a project are worth what it costs you to check them, so the commands are
here rather than the claims. Run them in a clone at any commit and you get whatever is
true at that commit, which may differ from what is printed below.

```sh
# implementation, excluding the test trees
find crates -name '*.rs' -not -path '*/tests/*' -print0 | xargs -0 cat | wc -l
# the test trees
find crates -path '*/tests/*' -name '*.rs' -print0 | xargs -0 cat | wc -l
# direct dependency surface, once the toolchain is installed
cargo tree --depth 1 -e normal
```

On 18 August 2026 the first printed 68,855 and the second 96,873, across 147 and 282
files in 15 crates. Our own ledger records 89,018 test lines for the same trees on the
same day, because it also excludes files merely named for tests. The tests are larger than
the thing they test under either rule, and that is the only claim the figures support.

Two counting rules, 7,855 lines apart, which is eight per cent of the larger, on one
repository, on one day. That is why the
command sits above the number and the date sits beside it. A figure without its method is
decoration.

This project is not small. It does not fit in an afternoon of reading, and the dependency
surface is real: the third command lists it, and every entry is code you would be trusting
on our recommendation.

## What this does not cover

Five classes of failure sit outside this, **by declaration rather than oversight**.
They are above the features because reading them first can save you the afternoon.

| out of scope | why it cannot be closed from the inside |
|:--|:--|
| Root or kernel-privileged writes | They bypass the tool entirely, and this build does not detect that |
| Writes into the tool's own state directory | A detector living in that directory cannot judge it. The defence is an artifact held elsewhere |
| A policy encoding the wrong intent | It is enforced faithfully. No verification reaches the question of whether the rule was right |
| Undoing one change and not another, across objects | Today the unit is a single transformation, and the check is a compare-and-set on the same object. If one change was made after reading another, nothing here records that it was read, so there is no way to ask for one back without the other |
| An issuer who cuts the tail off the chain | A hash chain proves that what you hold has not been edited. It cannot prove that what you hold is all there was. An issuer who hands you a genuine but older checkpoint, with the last entries removed, produces something that verifies. Detecting that needs a newer checkpoint from somewhere the issuer does not control, which is what an external anchor is for, and we do not publish to one yet |

**One thing worth knowing before you start rather than after.** What you can later select
on is fixed at the moment of capture, not at the moment you ask. A field that was not
recorded when the change landed cannot be recovered as a filter afterwards, so the set of
questions you can put to the history only ever grows forward from the day you begin.

The full list ships in [`docs/LIMITS.md`](docs/LIMITS.md), and a test fails if it drifts
from the code that enforces it. These are not sentences someone remembered to update.

## Where it stands

| | measured | under what conditions |
|:--|--:|:--|
| Test floor | **2,602** | probes across 454 suites, plus the SDK's 36 passed, 0 failed, 7 skipped (30 before the wasm bindings rebuild, E-SDK-9/10, the 25 August 2026 DR-46-39/40 vocab additions; 27 before that before the 2026-08-24 SDK sync, 25 before that before the E-SDK freshness tests) · frozen harness · fresh clone · one machine · single run · 25 August 2026 (req/801 G-07/G-08 + tamper-exit pin). Earlier printings said 1,838 across 324, then 2,073 across 362, then 2,089 across 365, then 2,211 across 390, then 2,224 across 393, then 2,229 across 396, then 2,235 across 397, then 2,240 across 400, then 2,253 across 401, then 2,254 across 401, then 2,256 across 401, then 2,257 across 401, then 2,258 across 403, then 2,261 across 403, then 2,318 across 407, then 2,326 across 408, then 2,333 across 409, then 2,348 across 410, then 2,363 across 414, then 2,381 across 415, then 2,405 across 420, then 2,415 across 422, then 2,439 across 423, then 2,443 across 424, then 2,445 across 424, then 2,449 across 425, then 2,451 across 425, then 2,483 across 432, then 2,498 across 433, then 2,509 across 434, then 2,511 across 434, then 2,519 across 436, then 2,521 across 437, then 2,525 across 438, then 2,532 across 439, then 2,538 across 440, then 2,549 across 443, then 2,553 across 444, then 2,558 across 445, then 2,562 across 446, then 2,587 across 450, then 2,595 across 451, then 2,601 across 454, each correct on the day and each overtaken since; this floor moves with every repair round |
| Machine-checked | **117** | **117 theorems, 12 of them counterexamples, 1 axiom, sorry 0** — theorems in Lean, out of 118 line-initial declarations in the Lean sources; the remaining one is an `axiom`, the statement assumed rather than proved, and it is named in the report. No `sorry`, the keyword standing in for a proof nobody wrote, so there are none. Proof rather than bounded model checking: nothing here is true only up to a scope · 18 August 2026 |
| Open holes | **0** | high severity, open as of 20 August 2026, out of the findings accepted in the audit ledger, counted as accepted findings whose repair has not been accepted. The twenty-eighth round found one; the repair was accepted the same day. This number was 3, then 0, then 1, then 0 again inside a week, and a twenty-ninth round is running now, so treat a zero here as the state of one afternoon rather than a property of the system — currency note 2026-08-25: rounds 29 through 44 have since landed, plus independent B-band and S(1) audits, all H=0 as recorded; the "one afternoon" caveat still applies, this is not read as a settled property — currency note 2026-08-25 (later same day): the B band's own 18/18-test count was independently live-re-run against a per-suite breakdown discrepancy an external audit had flagged; the re-run matched the claimed total by cancellation (two suites off by one in opposite directions) and reconfirmed H=0, so the band closes 8/8 (100 percent, the first band to) and the hold that had kept this number out of external material is lifted, 25 August 2026 |
| Not measured | **3** | Windows native, OneDrive, SMB — zero runs out of the three, as of 20 August 2026 (unchanged as of 2026-08-25) |

The commands that produce the first, second and fourth:

```sh
# test floor, and the SDK line separately
wsl -d Ubuntu-24.04 -- bash tools/e2e.sh
cd sdk/typescript && npm ci && npm test
# theorems, then the one assumed statement; add up the per-file counts
grep -rcE '^theorem' lean/GxSpec.lean lean/GxSpec/*.lean
grep -rcE '^axiom' lean/GxSpec.lean lean/GxSpec/*.lean
# the unmeasured surfaces, declared rather than discovered
grep -n "Windows, OneDrive" docs/LIMITS.md
```

**The third row has no command, and that is not an oversight.** An open-hole count is read
off the audit ledger — findings accepted as high severity whose repair has not been
accepted — and no single invocation produces it. Printing a command that did not really
generate the number would be the exact failure this table exists to avoid, so the row says
where the number comes from instead and you are taking that one on our word.

The third row is also the one worth reading twice. A count that only ever falls is a count
someone is managing rather than measuring.

**What would show this is wrong.** Two things, and either one is enough. Produce a
receipt that `gx receipt verify` accepts while the inverse it names does not restore the
state it claims to restore. Or land a change through the gate that leaves no receipt.
Both are checkable by someone who does not trust us, which is the point; if you find
either, open an issue and it will be recorded here whatever it costs us.

**Deliberately absent:** no build badge — continuous integration is switched off, and a
green tick would be a lie. No download counts, no star totals; neither measures whether
the thing works.

## What it does

Four behaviours, and the first thing worth knowing is that they are not four settings you
configure separately. One declaration in the catalogue names an object and what may happen
to it; the gate that runs before the action, the rule enforced while it runs, and the
fields attested after it are all read out of that same declaration. Nothing about that
shape is a discovery of ours, and it is being arrived at independently elsewhere. The
narrower difference is worth stating plainly: a receipt here carries an inverse that was
constructed and checked *before* the change landed, which is not the same thing as a field
reporting afterwards that an action was reversible.

**Escrow before commit.** Where an inverse can be constructed it is constructed, checked,
and stored durably *before* the change is applied. Undo is a checked property, not an
assumption made afterwards.

**Measured, not self-reported.** A fingerprint of the substrate is taken before and after
a change reaches the object a transformation names — so what happened is measured, not
described by the same process that did it.

**Offline-verifiable receipts.** Every verdict — admit, deny, escalate — is signed and
anchored in an append-only log, and re-checks with no network and no trust in the issuer.

**Declared coverage.** What is not covered ships beside what is, with equal weight. A skip
prints its name rather than passing quietly.

## Technical report

[`docs/TRACEFOLD_TR.md`](docs/TRACEFOLD_TR.md) is the long form: the calculus, the receipt
format, what was measured and under which conditions, related work graded by how well it
was checked, and every non-claim this project makes about itself.

## Contributing

Open an issue before a large change, and bring a measurement. The rules are short and are
in [`CONTRIBUTING.md`](.github/CONTRIBUTING.md); the shortest version is that a pull
request which lowers a count, skips a suite or narrows an assertion has to say so in its
own description. Silently bounded is the failure this project guards against hardest.

Good first things to pick up: a limit that is true but badly worded, a platform in the
"not measured" row above, or a self red-team probe that breaks something we believe holds.

Questions and half-formed ideas are welcome on [Discord](https://discord.gg/bFBvvg7AG).

## Sponsors

None, and none are being solicited yet. If this ends up load-bearing for your work and you
want it to keep being maintained, [say so](https://github.com/TraceFold/tracefold/issues) — knowing who depends on it changes
what gets prioritised more than money would at this stage.

## License

Apache-2.0 © Glovrex. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution of
incorporated work.
