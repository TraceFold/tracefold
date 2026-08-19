<div align="center">

<img src="https://github.com/TraceFold/tracefold/releases/download/brand-assets/banner.png" alt="Tracefold — verifiable reversibility" width="880">

### Undo is a feature. Reversibility is a property.

A change is held with a checked inverse **before** it lands, and every verdict becomes a
receipt anyone can verify offline — without trusting whoever issued it.

<p>
<a href="#what-this-does-not-cover"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/the%20limits%20first-ece7da?style=for-the-badge&labelColor=ece7da"><img alt="the limits first" src="https://img.shields.io/badge/the%20limits%20first-0b0a09?style=for-the-badge&labelColor=0b0a09"></picture></a>
<a href="#verify-a-receipt-without-us"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/verify%20a%20receipt-ece7da?style=for-the-badge&labelColor=ece7da"><img alt="verify a receipt" src="https://img.shields.io/badge/verify%20a%20receipt-0b0a09?style=for-the-badge&labelColor=0b0a09"></picture></a>
<a href="https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/technical%20report-ece7da?style=for-the-badge&labelColor=ece7da"><img alt="technical report" src="https://img.shields.io/badge/technical%20report-0b0a09?style=for-the-badge&labelColor=0b0a09"></picture></a>
<a href="https://discord.gg/bFBvvg7AG"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/discord-ece7da?style=for-the-badge&labelColor=ece7da&logo=discord&logoColor=0b0a09"><img alt="discord" src="https://img.shields.io/badge/discord-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=discord&logoColor=ece7da"></picture></a>
</p>

<p>
<a href="https://github.com/TraceFold/tracefold/blob/main/LICENSE"><img alt="license" src="https://img.shields.io/github/license/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
<a href="https://github.com/TraceFold/tracefold/commits/main"><img alt="last commit" src="https://img.shields.io/github/last-commit/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
<a href="https://crates.io/crates/tracefold"><img alt="crates.io" src="https://img.shields.io/crates/v/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09&logo=rust&logoColor=ece7da"></a>
<a href="https://www.npmjs.com/package/tracefold"><img alt="npm" src="https://img.shields.io/npm/v/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09&logo=npm&logoColor=ece7da"></a>
</p>

</div>

[Build](https://github.com/TraceFold/tracefold#build-it) ·
[Verify a receipt](https://github.com/TraceFold/tracefold#verify-a-receipt-without-us) ·
[Limits](https://github.com/TraceFold/tracefold#what-this-does-not-cover) ·
[Where it stands](https://github.com/TraceFold/tracefold#where-it-stands) ·
[What it does](https://github.com/TraceFold/tracefold#what-it-does) ·
[Report](https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md) ·
[Contributing](https://github.com/TraceFold/tracefold/blob/main/.github/CONTRIBUTING.md) ·
[Glovrex](https://glovrex.com)

---

> [!IMPORTANT]
> **Not released.** What sits on crates.io and npm under this name is a reservation — version
> `0.0.1`, holding the name, containing nothing you would want to install — and `gx wrap` has
> not shipped. The engine, the ledger, the policy gate, three substrate adapters, the CLI, the
> HTTP API, an SDK and a machine-checked model exist and are tested, but what you can install
> today is a build from source, and nothing below should be read otherwise.

## Build it

```sh
git clone https://github.com/TraceFold/tracefold
cd tracefold
cargo build --workspace
```

The binary is `gx` — short for Glovrex effects, the thing being governed. A project and its
command are allowed to have different names, the way ripgrep answers to `rg`. Rust stable;
the toolchain is pinned in `rust-toolchain.toml`. The
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

## What this does not cover

Three classes of failure sit outside this, **by declaration rather than oversight**.
They are above the features because reading them first can save you the afternoon.

| out of scope | why it cannot be closed from the inside |
|:--|:--|
| Root or kernel-privileged writes | They bypass the tool entirely, and this build does not detect that |
| Writes into the tool's own state directory | A detector living in that directory cannot judge it. The defence is an artifact held elsewhere |
| A policy encoding the wrong intent | It is enforced faithfully. No verification reaches the question of whether the rule was right |

The full list ships in [`docs/LIMITS.md`](docs/LIMITS.md), and a test fails if it drifts
from the code that enforces it. These are not sentences someone remembered to update.

## Who this is for

Someone who will later have to show a third party what an agent did, and to whom "we checked
our logs" is not an acceptable answer: an audit, a customer contract, a regulator, an internal
review. If you only want to undo your own mistake, the assistant you already use probably
keeps enough history, and you do not need this.

Two things an agent's own client cannot do for you, however good it gets.

It cannot see what left it. A rewind feature covers the conversation and the edits the client
made itself; a change that went out through a shell command, or through someone else's tool
server, was never inside its view.

And it cannot be the independent check on its own work. The party being audited grading its
own paper is not an audit, at any level of accuracy. That is a statement about position, not
about quality, so it does not resolve as the client improves.

Both gaps are the reason this exists. Neither is a complaint about those clients, which do
the thing they were built for well.

## Where it stands

| | measured | under what conditions |
|:--|--:|:--|
| Test floor | **1,770** | probes across 318 suites · fresh clone · one machine · single run |
| Machine-checked | **90** | theorems, 0 `sorry` · three axioms carried, not proved |
| Open holes | **3** | high severity · adversarial round 13 · repair in progress |
| Not measured | **3** | Windows native, OneDrive, SMB — zero runs |

Every figure can be re-derived here by someone who does not trust us. Figures without the
right-hand column are decoration, which is why that column exists.

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
"not measured" row above, or an adversarial probe that breaks something we believe holds.

## Sponsors

None, and none are being solicited yet. If this ends up load-bearing for your work and you
want it to keep being maintained, [say so](https://github.com/TraceFold/tracefold/issues) — knowing who depends on it changes
what gets prioritised more than money would at this stage.

## License

Apache-2.0 © Glovrex. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution of
incorporated work.
