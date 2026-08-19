<div align="center">

<img src="https://github.com/TraceFold/tracefold/releases/download/brand-assets/banner.png" alt="Tracefold — the inverse is sealed before the action" width="880">

### Undo, but proven.

An agent's change is held with a checked inverse **before** it lands, and every verdict
becomes a receipt anyone can verify offline — without trusting whoever issued it.

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

## Verify a receipt without us

This is the part that needs no build, no account, and no trust in whoever issued the
receipt. Given the binary and three files — the receipt, a checkpoint, a public key —
anyone can re-check the claim offline:

```sh
gx receipt verify --offline --checkpoint <checkpoint> --key <public key>
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

Four classes of failure sit outside this, **by declaration rather than oversight**.
They are above the features because reading them first can save you the afternoon.

| out of scope | why it cannot be closed from the inside |
|:--|:--|
| Root or kernel-privileged writes | They bypass the tool entirely, and this build does not detect that |
| Writes into the tool's own state directory | A detector living in that directory cannot judge it. The defence is an artifact held elsewhere |
| A policy encoding the wrong intent | It is enforced faithfully. No verification reaches the question of whether the rule was right |
| Undoing one change and not another, across objects | Today the unit is a single transformation, and the check is a compare-and-set on the same object. If one change was made after reading another, nothing here records that it was read, so there is no way to ask for one back without the other |

**One thing worth knowing before you start rather than after.** What you can later select
on is fixed at the moment of capture, not at the moment you ask. A field that was not
recorded when the change landed cannot be recovered as a filter afterwards, so the set of
questions you can put to the history only ever grows forward from the day you begin.

The full list ships in [`docs/LIMITS.md`](docs/LIMITS.md), and a test fails if it drifts
from the code that enforces it. These are not sentences someone remembered to update.

## Where it stands

| | measured | under what conditions |
|:--|--:|:--|
| Test floor | **1,838** | probes across 324 suites, plus the SDK's 18 passed, 0 failed, 7 skipped · frozen harness · fresh clone · one machine · single run · 18 August 2026 |
| Machine-checked | **91** | theorems in Lean (10 of them counterexamples). No `sorry`, which is the keyword that stands in for a proof nobody wrote, so there are none of those. One statement is assumed rather than proved, and it is named in the report · counted line-initially across the Lean sources, 18 August 2026. An earlier printing said 92 and two, which was a pattern-match counting too much |
| Open holes | **0** | high severity, open as of 18 August 2026. Not zero found: the eighteenth self red-team round found one, and it was repaired the same day, with nine gates red on the binary before the repair. A nineteenth round is in flight, so this number can go up |
| Not measured | **3** | Windows native, OneDrive, SMB — zero runs out of the three, as of 18 August 2026 |

Every figure can be re-derived here by someone who does not trust us. Figures without the
right-hand column are decoration, which is why that column exists.

**What would show this is wrong.** Two things, and either one is enough. Produce a
receipt that `gx receipt verify` accepts while the inverse it names does not restore the
state it claims to restore. Or land a change through the gate that leaves no receipt.
Both are checkable by someone who does not trust us, which is the point; if you find
either, open an issue and it will be recorded here whatever it costs us.

**Deliberately absent:** no build badge — continuous integration is switched off, and a
green tick would be a lie. No download counts, no star totals; neither measures whether
the thing works.

## What it does

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
