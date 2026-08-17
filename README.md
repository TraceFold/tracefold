<div align="center">

<img src="https://github.com/TraceFold/tracefold/releases/download/brand-assets/banner.png" alt="Tracefold — the inverse is sealed before the action" width="880">

### Undo, but proven.

An agent's change is held with a checked inverse **before** it lands, and every verdict
becomes a receipt anyone can verify offline — without trusting whoever issued it.

<p>
<a href="#what-this-does-not-cover"><img alt="the limits first" src="https://img.shields.io/badge/the%20limits%20first-0b0a09?style=for-the-badge&labelColor=0b0a09"></a>
<a href="https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md"><img alt="technical report" src="https://img.shields.io/badge/technical%20report-0b0a09?style=for-the-badge&labelColor=0b0a09"></a>
<a href="https://www.rust-lang.org/"><img alt="rust" src="https://img.shields.io/badge/rust-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=rust&logoColor=ece7da"></a>
</p>

<p>
<a href="https://github.com/TraceFold/tracefold/blob/main/LICENSE"><img alt="license" src="https://img.shields.io/github/license/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
<a href="https://github.com/TraceFold/tracefold/commits/main"><img alt="last commit" src="https://img.shields.io/github/last-commit/TraceFold/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09"></a>
<a href="https://crates.io/crates/tracefold"><img alt="crates.io" src="https://img.shields.io/crates/v/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09&logo=rust&logoColor=ece7da"></a>
<a href="https://www.npmjs.com/package/tracefold"><img alt="npm" src="https://img.shields.io/npm/v/tracefold?style=flat-square&color=0b0a09&labelColor=0b0a09&logo=npm&logoColor=ece7da"></a>
</p>

</div>

[Limits](https://github.com/TraceFold/tracefold#what-this-does-not-cover) ·
[Where it stands](https://github.com/TraceFold/tracefold#where-it-stands) ·
[What it does](https://github.com/TraceFold/tracefold#what-it-does) ·
[Build it](https://github.com/TraceFold/tracefold#building-it) ·
[Verify a receipt](https://github.com/TraceFold/tracefold#verifying-a-receipt-without-us) ·
[Technical report](https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md) ·
[Glovrex](https://glovrex.com)

---

> [!IMPORTANT]
> **Not released.** The crate is not on crates.io yet and `gx wrap` has not shipped. The
> engine, the ledger, the policy gate, three substrate adapters, the CLI, the HTTP API, an
> SDK and a machine-checked model all exist and are tested — but the thing you can install
> today is a `cargo build` from source, and nothing on this page should be read otherwise.

## What this does not cover

Three classes of failure sit outside this, **by declaration rather than oversight**.
Putting them above the features costs the first impression and saves you the afternoon you
would otherwise spend discovering them.

| out of scope | why it cannot be closed from the inside |
|:--|:--|
| Root or kernel-privileged writes | They bypass the tool entirely, and this build does not detect that |
| Writes into the tool's own state directory | A detector living in that directory cannot judge it. The defence is an artifact held elsewhere |
| A policy encoding the wrong intent | It is enforced faithfully. No verification reaches the question of whether the rule was right |

The full list ships in `docs/LIMITS.md`, and a test fails if it drifts from the code that
enforces it. These are not sentences someone remembered to update.

## Where it stands

| | measured | under what conditions |
|:--|--:|:--|
| Test floor | **1,770** | probes across 318 suites · fresh clone · one machine · single run |
| Machine-checked | **90** | theorems, 0 `sorry` · three axioms carried, not proved |
| Open holes | **3** | high severity · adversarial round 13 · repair in progress |
| Not measured | **3** | Windows native, OneDrive, SMB — zero runs |

Every figure can be re-derived from this repository by someone who does not trust us.
Figures without the right-hand column are decoration, which is why that column is here.

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
anchored in an append-only log. A receipt re-checks with no network and no trust in the
issuer.

**Declared coverage.** What is not covered ships beside what is, with equal weight. A skip
prints its name rather than passing quietly.

## Building it

```sh
git clone https://github.com/TraceFold/tracefold
cd tracefold && cargo build --workspace
```

The binary is `gx`. The subcommands that exist today are `submit`, `plan`, `verify`,
`commit`, `undo`, `cancel`, `escalation`, `receipt`, `replay`, `log`, `key`, `policy` and
`serve`. Verifying a receipt needs nothing but the binary and three files — no workspace,
no key store, no network:

## Verifying a receipt without us

Checking a receipt needs the binary and three files. No workspace, no key store, no
network, and no account with us.

```sh
gx receipt verify --offline --checkpoint <checkpoint> --key <public key>
```

It exits `0` when the receipt holds and `7` when it does not.

## Technical report

[`docs/TRACEFOLD_TR.md`](docs/TRACEFOLD_TR.md) is the long form: the calculus, the receipt
format, what was measured and under which conditions, related work graded by how well it
was checked, and every non-claim this project makes about itself.

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). Built by
[Glovrex](https://glovrex.com).
