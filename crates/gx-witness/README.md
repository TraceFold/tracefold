# gx-witness

Provenance, evidence and DSSE-signed receipts (41 §2 / 42 §3.9-§3.10).

## What this crate guarantees

Quoted from `src/lib.rs`'s crate-level doc comment:

> P-7's witness: after the gate has decided, something has to be able to say *afterwards, to a
> third party, offline* what was decided and over what. That is a receipt — a DSSE envelope
> over a canonical `ReceiptPayload` — and the provenance and evidence it refers to.

> FR-017 asks gx-witness for a `Proof` while 42 §0 files the nearly identical `ProofRef` under
> gx-gate (M3). **E-M2-12** ... put the family in gx-core so the two crates share one set of
> types, and this crate satisfies FR-017's MUST by re-exporting them rather than by defining a
> second spelling.

## What this crate does not guarantee

> **What this crate must never be said to prove.** 46 §2.5's T4 (receipt soundness) and T5
> (witness lax composition) are **not proved**: `Receipt.lean` does not exist. A receipt this
> crate issues is a signed record of what the engine decided. It is not backed by T4, and req/49
> §1 N-10 marks saying otherwise as the overclaim 45 §4.1 forbids.

## Position

`req/spec/40-architecture/41-architecture.md` §2 (where this crate sits, what it may depend
on); `42-*.md` §3.7 / §3.9 / §3.10 (field tables); `32-functional.md` FR-015..FR-020;
`34-*.md` AC-015..AC-020 and AC-070.

## Not covered

A signed receipt is not a proof of T4 (receipt soundness) or T5 (witness lax composition) —
neither is proved in Lean for this crate; see "What this crate must never be said to prove"
above.
