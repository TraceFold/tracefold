# Verifying an AI agent's actions offline

To verify what an AI agent changed, offline, you need three files: the signed receipt of the
change, a signed checkpoint of the ledger head, and the issuer's public key. `gx receipt
verify --offline` checks them on a machine with no network and no copy of the project, exits
`0` only when the receipt holds, and requires no trust in the agent, the operator, or the
machine that issued the receipt.

That's the claim this repository exists to make checkable. This page explains where the
receipt comes from, why the tool sometimes refuses to act at all, and what an honest audit
trail for AI agent actions can and cannot contain. The companion page,
[Flip one byte and the verifier exits 7](tamper-evident-receipts.md), is the executable half:
every command there was run on a fresh clone (commit `cf143ac3`, 2026-08-26) and the exit
codes shown are the ones observed.

## Attach at the interface, not inside the agent

`gx` doesn't live inside a model, a prompt, or an agent framework. It sits at the interfaces
where changes actually land: a filesystem, a git repository, an MCP server. Agent frameworks
change shape every few months; the interfaces they write through are old and stable, and a
guard attached there doesn't care which framework, which model, or which vendor did the
writing.

The unit of observation is a transformation, not a session and not a conversation. Before a
change is applied, the substrate adapter reads the object the change names, fingerprints its
current state, computes the delta that would be applied, and builds the inverse of that
delta. The inverse is escrowed first. Only then is the change applied, and what gets signed
is the whole story: what the object was, what was done to it, under which policy decision,
reversible by what.

Concretely, in the six verbs of the walkthrough: `submit` describes the change without
making it, `plan` reads the object and fixes the transformation's identity, `verify` asks
the gate, `commit` applies it and issues the signed receipt, `undo` commits the escrowed
inverse as a new transformation, and `receipt verify --offline` lets a third party check any
of it later. History only grows forward; an undo is itself a receipted change that names what
it took back, and nothing is rewritten.

## An adapter is admitted by contract, not by claim

This repository carries three substrate adapters: `gx-adapter-fs`, `gx-adapter-git` and
`gx-adapter-mcp`, all under `crates/`. Next to them sits `gx-substrate-conformance`, a
harness of seven contracts that every adapter has to pass by inheriting the same test
battery. The contracts are the interesting part of the design, because they're what makes
"supported substrate" a checked property instead of a bullet point: an adapter has to prove
its snapshots report the state it was asked about, that apply-then-undo returns the object to
where it started, and, in the contract that carries the most weight here, that when an
inverse cannot be built the adapter answers "none", as a fact, rather than erroring or
improvising one.

A fourth substrate counts when it passes those seven contracts. That's the honest count
today: three, not "any substrate". It's also the distribution story, stated plainly: every
interface that solidifies in front of AI agents is a place this discipline can attach, and
the cost of attaching is exactly the cost of proving the contract, adapter by adapter.

## Conformance means stating correctly what you cannot take back

The least obvious design decision is that refusal is a feature. When `gx` cannot build the
inverse of a change, it doesn't record-and-proceed. It stops, says so, and the refusal is a
signed record with its own exit code: `2` when policy denies, `4` when a person has to rule
first. The tutorial documents the one way to reach an escalation on the filesystem adapter in
v0.1: overwriting a file whose current contents are too large to escrow. `gx` will not make a
change nobody can take back and call it undoable.

This is what conformance actually measures. A tool in this position is judged not by how much
it claims to reverse but by whether it describes its own boundary correctly: what it can
invert, what it can only witness, and what it refuses. A receipt for an irreversible change
that pretends otherwise would verify beautifully and mean nothing. The adapters are therefore
required to be honest before they're allowed to be useful, and the same posture runs through
the repository's own documentation: the limits sit above the features in the
[README](../../README.md), and [`LIMITS.md`](../LIMITS.md) is drift-tested against the code
that enforces it.

## What the audit trail holds, and what it can't

Each admitted change leaves a signed receipt in `.gx/receipts/`, appended to a ledger with
inclusion proofs, and `gx log checkpoint` publishes a signed head as a plain file, so the
check travels: the offline verifier needs the checkpoint you hold, not the machine that holds
the ledger. The check verifies the signature, the canonical identity of the transformation,
and the receipt's inclusion under that checkpoint.

What it can't do is also part of the record. Root-privileged writes bypass the tool entirely.
A policy that encodes the wrong intent is enforced faithfully. An issuer who cuts the tail
off the ledger hands you something that still verifies against its own shortened head, which
is why the checkpoint wants anchoring outside the issuer, and why that is stated as an open
limit rather than papered over. If an audit trail for agent actions doesn't tell you where
it's blind, you're reading marketing.

Not released; build from source. The walkthrough with every command and every observed exit
code is [here](tamper-evident-receipts.md), and the design in full is in the
[technical report](../TRACEFOLD_TR.md).
