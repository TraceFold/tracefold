# Tracefold

Tracefold escrows a verified inverse before a change reaches a system, and turns every verdict into a receipt a third party can check without trusting the issuer or reaching the network.

## Quickstart

All three commands below are **coming soon**. Tracefold is pre-publication: the crate is not on crates.io, and the MCP wrap command has not shipped yet (see [Status](#status)). Nothing here is presented as working today.

```sh
# Coming soon: not yet published to crates.io.
cargo install tracefold

# Coming soon: the MCP wedge (`wrap`) is the next milestone, not the current one.
gx wrap <mcp-server-command>

# Coming soon.
gx demo
```

The CLI binary is `gx`. The commands that exist today (`submit`, `plan`, `verify`, `commit`, `undo`, `cancel`, `escalation`, `receipt`, `replay`, `log`, `key`, `policy`, `serve`) are built from this repository but not yet distributed as a package; build from source with `cargo build --workspace` in the meantime.

## What it does

Four things, and the technical report (below) is the long form of each:

- **Escrow before commit.** Where an inverse can be constructed, it is constructed, verified, and stored durably *before* the change is applied. Undo is a checked property, not an assumption made after the fact.
- **Measured, not self-reported.** A fingerprint of the substrate is taken before and after a change reaches the object a transformation names, so what happened is measured rather than described by the same process that did it.
- **Offline-verifiable receipts.** Every verdict (admit, deny, or escalate) is signed and anchored in an append-only log. A receipt can be re-checked by a third party with no network connection and no trust in whoever issued it.
- **Declared coverage.** What the system does not cover ships next to what it does, with equal weight. A skip prints its name instead of passing quietly.

## Limits

Read this section before the one above it. What follows is not the full threat model (the technical report's §4 is), but it is the same eight lines, first.

1. If the policy and the intent diverge, Tracefold enforces the divergent policy faithfully. This is the oracle problem, and it is structural, not mitigated.
2. Out-of-band writes by root or kernel-privileged actors are not detected. Detection (not prevention) is roadmap work for v0.3+.
3. Key revocation cannot cover the time a key was actually compromised, and a timestamping authority does not close this gap.
4. An MCP tool call's effect landing outside the object a transformation is about cannot be observed: the protocol does not report it. What we do instead is serialise calls per server.
5. Lexical normalisation of a locator does not cover alias resolution inside the substrate (symlinks, refs, server-side aliases).
6. A verdict-count checkpoint closes non-disclosure only. A policy relaxed until it refuses nothing reports a truthful zero, and it does not detect a split view shown to two different verifiers.
7. Continuous differential testing against a formal (Lean) model is planned for milestone M8. It does not exist today.
8. Machine-enforced CI on push and pull request covers 2 of 13 crates (`gx-core`, `gx-canon`). The rest of the green is a record of a person running the full script by hand.

## Status

Milestones M1 through M7 are complete: the engine (a state machine with 21 named transitions), canonical encoding, receipts, the append-only ledger, the policy gate, three substrate adapters (filesystem, Git, MCP), the CLI, and the HTTP API. Test floor at milestone M7 in the development tree: 1,370 probes across 247 suites (see the technical report, §5, for the conditions attached to every number). Seventeen suites in that count measure agreement with a private requirements corpus and a private conformance-probe tree that this public repository does not carry; they are not included here — see [`docs/DEVELOPMENT_TREE_TESTS.md`](docs/DEVELOPMENT_TREE_TESTS.md). This repository's own floor, 2026-08-13: **1,160 passed / 216 suites, 0 failed.**

Next: the MCP wire and checkpoint surface, request authentication, and a TypeScript SDK. A formal (Lean) model and continuous differential testing are planned for milestone M8 and have zero progress today: no `lean/` directory, no differential-test corpus, and no artifacts for the acceptance criteria that would cover them.

## Technical report

[`docs/TRACEFOLD_TR.md`](docs/TRACEFOLD_TR.md) is the long form: the calculus, the receipt format, what was measured and under what conditions, related work with sources graded by how well they were checked, and every non-claim this project makes about itself.

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
