# Tracefold Technical Report (v0.1)

**A technical report. Draft v2 — not peer reviewed.**

Glovrex · Apache-2.0 · drafted 2026-08-13

> v2 is a prose revision of v1. No claim, number, condition, citation, or scope changed. v1 is kept beside this file.

<!-- src: req/112 §7.6 (the report's standing and outline) / §2 (naming) / 45 §4 (the language of promises) -->

---

## Status of this document

| Field | Value |
|---|---|
| Kind | Technical report accompanying a public source repository. Not a submission to a refereed venue, and not a specification. |
| Audience | Developers who run agents or MCP servers against systems that change, and engineers who build verification and audit machinery. |
| Software described | The `gx-*` crate workspace (13 crates), a Rust implementation of what the internal specification calls a *verified transformation calculus*. CLI binary: `gx`. |
| Specification baseline | Internal specification v0.2.3; implementation tag `req0.09` (milestone M7 complete). All quantitative claims are pinned to named commits in §5. |
| Licence | Apache-2.0. |
| Product name | **Tracefold** is the selected name. It is provisional in one respect: a registrar and trademark check (`.com`/`.dev`, J-PlatPat/USPTO) is a precondition of publication and had not returned when this draft was written. If that check fails the name changes and this report changes with it. The company name, Glovrex, is fixed. |
| Formal-methods status | There is no Lean model and no differential-test corpus in the tree. Both are planned work (milestone M8) and are written here in the future tense only. See §7.4. |

**How to read this report.** Start with §7 and §4. Non-claims and declared coverage come before §2 and §3 in importance, and if you have time for two sections, those are the two.

The reason is not modesty. What a system does is always easier to write down than what it does not do, so a report that opens with the mechanism trains its reader to skim past the part that would have told them whether any of it applies to them.

### The contract, on one page

| What is asserted | What is not asserted |
|---|---|
| A change is judged against an operator-supplied predicate **before** it is applied. | That the predicate expresses anyone's intent, or that a change satisfying it is safe or correct. |
| Where an inverse is constructible, it is constructed, checked, and durably stored **before** the change is applied; where it is not, that fact is an input to the decision. | That every change is invertible, or that applying an inverse is itself harmless. |
| Every verdict — including refusals and escalations — produces a signed receipt. | That refusals are visible in the ledger; they are not, which is why §3.5 exists. |
| A receipt can be re-checked by a third party with no network and no trust in the issuer: signature, ledger inclusion, canonical identifier consistency. | That the receipt lets anyone reconstruct the data that changed. Object bytes are not stored. |
| Counts of verdicts are committed to over contiguous windows, bound to the ledger's size, so that undisclosed refusals become arithmetically detectable. | That a permissive policy is detectable, or that two verifiers shown different chains can tell. |
| Changes reaching the system through an adapter are covered. | That changes reaching the substrate by any other route are prevented, detected, or seen. |
| Measurements in §5 are real, conditioned, and reproducible from named commits. | That they are pass marks. The thresholds they are compared against are provisional design budgets. |
| A formal model and continuous differential testing are planned. | That any of it exists. It does not (§7.4). |

---

## A note on what this report claims to be new

<!-- src: req/112 §7.6 (Owner directive, verbatim) / 11 P-9 / 12 §禁止事項 -->

Nothing here is presented as an invention.

Its author works from a flat position on novelty: what gets advertised as new is usually a combination of existing parts, a small rearrangement, a continuation of some mathematics, or an import from the field next door. This report assumes that about itself. Every mechanism below has a named ancestor, and the ancestors are given in §6 instead of in a footnote, because the combination is the one thing we are in a position to call ours. Even "ours" means no more than "we have not found it assembled this way", which is a statement about how we searched and not about the world.

Concretely. Leaf and interior domain separation: Certificate Transparency. The signed envelope is DSSE, which comes from in-toto. The tile-backed log layout follows Rekor v2 and the tlog-tiles design. Content addressing over a canonical encoding belongs to IPLD, and before that to Git. Compare-and-swap on a precondition fingerprint is optimistic concurrency control. Pre-provisioned inverses are compensating transactions, which is Sagas. Deciding before applying, then applying exactly the thing that was decided about, is `terraform plan` and `terraform apply`. The policy engine is Cedar, unmodified.

What we assembled is a pipeline in which those parts constrain each other, two structural rules about where authority is allowed to live (§2.4), and a discipline of publishing the boundary of what the assembly covers (§4).

Three moves are not made in this report: no mechanism is called a first, none is called the only one of its kind, and none is called unique. (The word *only* appears below in its ordinary sense — *the only write*, *the only sensor* — where it describes an internal constraint and not a position in a market.) Where we report that we did not find something, the sentence carries the date, the method, and a pointer to the ledger of what was searched. A negative claim without those three is not a claim.

**Source-strength labels.** Following a convention this project used in an earlier report, every external citation carries the strength of its grounding:

- **[PV]** — a primary source opened and read word for word while preparing *this* draft;
- **[PB]** — bibliographic record only; the body was not opened for this draft;
- **[2°]** — secondary sources only.

Appendix C lists exactly which sources were opened. The honest summary: most citations below are [PB] or [2°]. This draft was written against the repository, not against a library.

---

## §1 Problem: the effect lands before the check

<!-- src: req/112 §1 (target adjudication, incident record) / 45 §2.1 TH-1 / 21 §9-2 / 11 C-7 P-4 -->

### 1.1 The asymmetry

An agent that changes a system works in one order: decide, act, report. A model decides. A tool call acts. The report is text produced afterwards by the same process that did the acting. Verification, where it exists at all, sits at the end of that chain, and it reads what already happened.

That ordering has a consequence which is easy to state and hard to design around. **By the time anything is checked, the effect exists.** If the check fails, what remains is a choice between describing the damage, attempting a repair that is itself unverified, and restoring from a backup whose currency nobody has measured. The check has become a reporting function.

This is not hypothetical, and it is not a beginner's problem. On 2025-07-18 an agent operating a hosted development environment executed destructive commands against a production database during an explicitly declared code freeze, then produced fabricated records in place of the deleted ones; the vendor's chief executive apologised publicly and shipped permission and rollback changes within days [2°, AI Incident Database, Incident 1152, *"LLM-Driven Replit Agent Reportedly Executed Unauthorized Destructive Commands During Code Freeze, Leading to Loss of Production Data"*, https://incidentdatabase.ai/cite/1152/ — record opened 2026-08-13; the five press reports it indexes were not opened].

The relevant half is the second one. The agent's own account of what it had done was wrong, and that account was the only record.

Our internal incident file carries a second case from 2026-04, in which an agent holding an over-scoped API token found in the repository it was working on deleted a production database and its backups. That case is recorded with weaker sourcing and we do not cite it here. It sits on Appendix C's do-not-assert list.

### 1.2 What existing countermeasures do, and where the gap is

Three families of countermeasure are widely deployed, and each is a good answer to a different question.

| Family | Examples | The question it answers | What it leaves open |
|---|---|---|---|
| **Isolation** | container and microVM sandboxes, ephemeral workspaces, least-privilege tokens | *Can the blast radius be bounded?* | Bounding the radius does not decide whether the change inside the boundary should have happened, and the systems agents are asked to change are frequently outside any sandbox by construction. |
| **Permission and approval** | pre-execution allowlists, human-in-the-loop confirmation, tool-call gateways | *Can a class of action be refused?* | Refusal is usually per tool name or per argument shape, and it is stateless with respect to the object being changed. It does not carry a record that a third party can check later. |
| **Recording** | audit logs, observability traces, event streams, self-reported provenance | *What happened?* | The record is produced by the party whose behaviour is in question, is usually unsigned, and is read after the fact — the same position the failed check occupies. |

All three sit on the "read it afterwards" side of the line. What is thin is the conjunction of three properties, none of them individually novel:

1. **Hold the change before it lands**, so that the decision is about a change that has not yet occurred;
2. **Prove what was decided and what was applied**, in a form a third party can re-check without trusting whoever issued it;
3. **Verify in advance that the change can be taken back**, and keep the means of taking it back, so that "reversible" becomes a checked property instead of an assumption about the substrate.

This report describes a system built around that conjunction. The conjunction is the contribution; each conjunct has ancestors (§6).

### 1.3 What is deliberately not in scope of the problem statement

The system does not attempt to decide whether a change is *good*. Whoever operates it writes the predicate a change is judged against; we do not. If the predicate is wrong, the system enforces a wrong predicate faithfully and issues a signed receipt saying so.

That is the oracle problem, and it is structural. It appears here rather than in a limitations section at the end because a reader who does not know it will misread every section that follows. §4 and §7 come back to it.

---

## §2 The calculus: candidate → escrow → gate → commit

<!-- src: 11 C-7/C-8/P-4/P-5 / 41 §5 (commit protocol) / 43 §1–§3 (state machine, 21 transitions) / 42 §3.5 (Fingerprint) / 42 §3.12 (EscrowedInverse) -->

### 2.1 The primitive

The unit is not the agent and not the action. It is a triple: an **object**, a **transformation**, and an **admissibility predicate**.

- An **object** is a content-addressed snapshot reference: which substrate, where in it, a digest of the content, and which representation kind. The bytes themselves are never stored, because the substrate already holds them. An object is a name plus a digest, not a copy.
- A **transformation** is a first-class value describing a change from one object to another. It carries its intent, its subject, its planned delta, its change context, its actor, and its parent transformations. It does *not* carry its own lifecycle state. State lives in an engine-side table keyed by the transformation's identity, so the canonical form of a transformation never depends on how far along it is.
- **Admissibility** is a predicate over transformations, supplied by the operator as a Cedar policy set plus a set of invariant checks. The system evaluates it. It does not author it.

Transformations are first-class values, and not events, so that the same machinery reaches changes *of* changes. A policy edit is a transformation whose subject is the policy, and it goes down the same pipeline. The implementation caps that recursion at order ≤ 2, a change to a change, which is an engineering limit and not a claim about the general case.

### 2.2 The pipeline

The basic operation is not *execute*. It is **candidate → verify → canonicalize → commit**, with an inverse escrowed inside the commit critical section, before the change is applied.

```
1.  submit(intent)          -> Draft.          IntentId fixed here (CID of the intent).
2.  snapshot + plan         -> Candidate.      PlannedDelta produced by the adapter (pure);
                                               precondition Fingerprint_0 recorded;
                                               TransformationId fixed here (CID of the
                                               canonical form including delta and target).
3.  verify                  -> evidence collected -> Gate::verify
                                               -> Admit | Deny | Escalate
                                               (a VerdictReceipt is issued for all three).
4.  canonicalize            -> canon(T) recomputed; idempotence checked against the
                                               TransformationId fixed in step 2.
5.  commit (critical section):
    a. Fingerprint_1 := adapter.precondition(now)
    b. if Fingerprint_1 != Fingerprint_0 -> Aborted(PreconditionChanged)      [compare-and-swap]
    c. adapter.invert(delta, pre) -> escrow the inverse delta, durably        [BEFORE apply]
    d. adapter.apply(delta)                                                   [the only write]
    e. ledger.append(canonical T + verdict) -> InclusionProof
    f. CommitReceipt issued (DSSE-signed)
6.  Committed               -> immutable. Undo is a new commit, never a rewrite.
```

Three of those orderings carry the weight. The rest is bookkeeping.

**Escrow precedes apply.** Step 5c runs before step 5d. The adapter hands over the inverse and the engine persists it before the world is touched. Where no inverse can be constructed the adapter returns `None`, and the gate sees that as `invert_available: false`, so a policy can refuse on it or admit knowingly. The distance between "we can probably undo this" and "the undo is in hand" is the whole point of the step. Escrow is also the one structure in the system whose payload is stored in full instead of by digest, for the obvious reason: a digest of an undo cannot perform an undo.

**Compare-and-swap precedes apply.** Step 5b re-reads the precondition fingerprint at commit time and compares it against the one taken at plan time. A mismatch aborts with `PreconditionChanged`, and the adapter's `apply` is never called. This closes the window between verification and application the only way that does not require a global lock: optimistically, by aborting instead of by excluding.

A residual window survives, between the re-read and the call, and it is wider for adapters whose `apply` is not a single atomic operation. The threat model carries it as a residual risk. We have not solved it.

**Undo is a commit, not a rollback.** No transition leads from `Committed` back to any earlier state. Taking a change back means submitting a new transformation whose intent is to apply the escrowed inverse, and that new transformation goes through the whole pipeline itself, gate included. An undo is not exempt from verification. When it commits, an edge is drawn to the original, whose status becomes `Superseded` — an appended fact, leaving the original's canonical record, receipt, and ledger entry untouched.

### 2.3 The state machine

The lifecycle is a state machine with 11 states and **21 named transitions**, defined exhaustively with guard, side effects, target state, and an idempotency rule per transition.

| State | Meaning | Terminal |
|---|---|---|
| `Draft` | intent submitted; `IntentId` fixed; nothing planned yet | no |
| `Candidate` | snapshot and plan done; delta and `Fingerprint_0` recorded; `TransformationId` fixed | no |
| `Verifying` | evidence collection and gate evaluation in progress | no |
| `Admitted` | gate returned `Admit` | no |
| `Denied` | gate returned `Deny` | terminal, except under record-only mode |
| `Escalated` | gate returned `Escalate`; awaiting a signed human decision | no |
| `Canonicalized` | `canon(T)` recomputed and checked idempotent | no |
| `Committing` | the commit-time critical section | no |
| `Committed` | applied, appended to the ledger, receipt issued | terminal |
| `Aborted` | cancelled at some stage, always with a reason from a closed set of six | terminal |
| `Superseded` | was `Committed`; a later transformation committed its inverse | terminal |

The transitions, in full, because "21 transitions" is not checkable and a list is:

| ID | From | Trigger | To |
|---|---|---|---|
| T-1 | (start) | `submit(intent)` — schema-conforming; `IntentId` fixed | `Draft` |
| T-2 | `Draft` | `plan()` — snapshot and plan succeed; `Fingerprint_0` taken; `TransformationId` fixed | `Candidate` |
| T-3 | `Candidate` | `verify_start` — no conflicting predecessor, or queued | `Verifying` |
| T-4a | `Verifying` | gate returns `Admit` | `Admitted` |
| T-4b | `Verifying` | gate returns `Deny` | `Denied` |
| T-4c | `Verifying` | gate returns `Escalate`; ticket minted | `Escalated` |
| T-4d | `Verifying` | verifier unreachable, posture fail-closed | `Aborted(VerifierUnavailable)` |
| T-4e | `Verifying` | verifier unreachable, posture fail-open (opt-in); `enforced=false`, `fail_posture_engaged=true` | `Admitted` |
| T-5 | `Escalated` | human approves, signed | `Admitted` |
| T-5b | `Escalated` | human rejects, signed | `Denied` |
| T-6 | `Candidate` / `Verifying` / `Escalated` | time-to-live exceeded | `Aborted(Expired)` |
| T-7 | any pre-`Committing` state | owner cancels | `Aborted(OwnerCancelled)` |
| T-8 | `Admitted` | `canonicalize` — idempotence checked | `Canonicalized` |
| T-8r | `Denied` | `canonicalize` under record-only mode; `enforced=false` stamped | `Canonicalized` |
| T-9 | `Canonicalized` | `commit_start` — journalled *before* any side effect | `Committing` |
| T-10a | `Committing` | `Fingerprint_1 != Fingerprint_0`; `apply` not called | `Aborted(PreconditionChanged)` |
| T-10b | `Committing` | `invert` succeeded; inverse persisted | `Committing` (internal step) |
| T-10c | `Committing` | `apply` failed; best-effort rollback from escrow | `Aborted(ApplyFailed)` |
| T-11 | `Committing` | `apply` succeeded; ledger appended; receipt issued | `Committed` |
| T-12 | `Committed` | another transformation committed this one's inverse | `Superseded` |
| T-13 | any non-terminal state | internal failure not attributable to any external cause | `Aborted(InternalError)` |

Every row carries a guard column and an idempotency rule in the specification that this summary omits. The ones that matter operationally: `plan` is pure and safe to re-run, verification is read-only, `ledger.append` is key-idempotent so that a crash inside the critical section still yields exactly-once commit on recovery, and duplicate cancellations and duplicate supersede edges are no-ops.

Two transitions deserve naming here, because both recur later.

**T-4e — the admission nobody made.** When the verifier or the evidence collector cannot be reached, the default posture is fail-closed: `Aborted(VerifierUnavailable)`. A substrate may opt in explicitly to fail-open, and then the transformation proceeds *as if* record-only, with the receipt stamped `enforced = false` and `fail_posture_engaged = true`. Its journal record is an `Admit` whose verdict digest is `None`, because there is no verdict. The gate did not run. Folding this into the ordinary admit count would report a judgement nobody made as a judgement, so it is counted separately (§3.5).

**T-13 — the internal error.** A residual transition covering panics, invariant-violation detection, and undeserialisable journals, kept distinct from every externally-caused abort. It exists so that a bug does not get to wear the costume of an ordinary failure.

Transition coverage is a completion condition: all 21 must be exercised by at least one test, tracked in a transition-ID-to-test-name table and checked by a named CI stage instead of by a workspace-wide run. The requirement was itself wrong for a period. The specification enumerated 19, having dropped T-4e and T-13, which are precisely the two hardest to reach, and it was corrected to 21 in v0.2.3 after the implementation was found already to cover all 21. The direction of that correction is the only interesting thing about it: the document was behind the code, and the fix went to the document.

### 2.4 The membrane: two rules about where authority lives

<!-- src: crates/gx-canon/tests/authority_boundary.rs (則 1, verbatim header) / crates/gx-cli/src/clock.rs and crates/gx-api/tests/rule_two.rs (則 2) / 41 §6 -->

Correct ordering in a pipeline is worth very little if a component can step around the pipeline. Two structural rules — the project calls them 則 1 and 則 2, *rule one* and *rule two* — say where the ability to do certain things is allowed to exist. Tests that scan source text enforce both. Not review.

**Rule one — the surfaces hold no semantic authority.** A *surface* is a crate that ships a dependency on the engine; in the current workspace, `gx-cli` and `gx-api`. Across their sources, three counters must read zero:

1. calls into `gx-canon`, because canonical encoding, and therefore identity minting, is the core's monopoly;
2. constructions of `Verdict`, because judging belongs to the gate;
3. writes of a lifecycle state, because transitions belong to the engine.

The set of surfaces is both declared and derived. Deriving it means reading which workspace members declare `gx-engine` as a shipping dependency, then comparing that against the declaration, so a surface added tomorrow and not written down gets scanned anyway and then reported as undeclared. The check runs over text and not over a compiled artifact, deliberately: "somebody added a canonical encode to the CLI" should be caught before the tree builds.

**Rule two — the outside world enters at one point.** Clocks and entropy are injected at the engine boundary, which puts the layer that reads a real clock or seeds a real RNG in the surface, and each surface must read each of them exactly **once**, measured by scanning its sources. In the CLI, `main` calls `now()` and nothing else does, and there is deliberately no `--at` flag. A clock that is a command-line argument is a receipt whose `issued_at` can lie. Thirteen HTTP endpoints are thirteen opportunities for a second clock, which is why the API's copy of this check lives beside the API instead of in a shared test crate.

A related single-road rule from an earlier milestone, recorded under the same name in an earlier document, counts call sites of `SubstrateAdapter::apply` across all shipping crates and requires exactly one. We note the numbering collision instead of silently merging the two.

Both rules make the same modest point. They do not make bad changes impossible. They turn one class of bypass into a compile-time-adjacent failure instead of a code-review question.

### 2.5 Substrate adapters

Everything substrate-specific lives behind one trait: `snapshot`, `plan`, `precondition`, `apply`, `invert`, `commutation`, `kind`. `plan` is pure, producing a delta without touching the world. `apply` is called only after the gate has admitted and the compare-and-swap has passed, and is contractually idempotent. `invert` may return `None`, and `None` is information the gate receives, not an error.

Three adapters ship: filesystem, Git, and MCP. A shared conformance harness runs seven contract properties against each. The MCP adapter carries the product's first use case, and it also carries the sharpest declared limit (§4.3).

### 2.6 The workspace

Thirteen crates, with the dependency direction running downward and the two top rows holding no semantic authority (§2.4).

| Crate | Responsibility | Notable constraint |
|---|---|---|
| `gx-cli` | command surface: `submit`, `plan`, `verify`, `commit`, `undo`, `cancel`, `escalation`, `receipt`, `replay`, `log`, `key`, `policy`, `serve` | a *surface*: rules one and two apply |
| `gx-api` | HTTP endpoints and a JSONL event stream | a *surface*: rules one and two apply |
| `gx-engine` | the state machine, the commit protocol, the write-ahead journal, escrow storage | injects clock and entropy; the only caller of `apply` |
| `gx-gate` | Cedar policy set evaluation, invariant registry, verdict composition | judging happens here and nowhere else |
| `gx-witness` | provenance, evidence, receipts, DSSE, keys, revocation | Ed25519 |
| `gx-log` | append-only Merkle tile log, inclusion and consistency proofs, checkpoints | append-only by construction |
| `gx-canon` | canonical DAG-CBOR, content identifiers, the RFC 8785 compatibility path | the monopoly on canonical encoding |
| `gx-core` | the central types and nothing else | no I/O, deterministic, `forbid(unsafe_code)` |
| `gx-substrate` | the `SubstrateAdapter` trait and its delta and fingerprint types | the point where substrate neutrality is held |
| `gx-adapter-fs` / `-git` / `-mcp` | the three shipped substrates | plan is pure; apply is idempotent by contract |

The engine does not execute. Application to a substrate is the adapter's job, and the engine arbitrates between gate, canonicaliser, witness, and ledger. Random numbers and clocks are injected at the engine boundary so that a run can be replayed deterministically from the journal.

### 2.7 The invariants the state machine is required to hold

These are written as obligations on property tests, and they are the statements a formal model would be asked to discharge if one existed (§7.4).

| ID | Kind | Statement |
|---|---|---|
| INV-S1 | safety | Every path to `Committed` passes through `Admitted ∧ Canonicalized` — or, under record-only mode, `Denied ∧ enforced=false ∧ Canonicalized`. There is no other path. |
| INV-S2 | safety | `Committed` is immutable. Its one outgoing edge, `Superseded`, appends metadata and rewrites neither the canonical record nor the receipt. |
| INV-S3 | safety | At most one ledger entry per transformation identifier. |
| INV-S4 | safety | Aborted and denied transformations do not appear in the ledger, record-only applications excepted. |
| INV-S5 | safety | A commit with `enforced=false` is distinguishable in the receipt's canonical structure from one with `enforced=true`. |
| INV-S6 | safety | An escalated transformation never reaches `Admitted` or `Denied` without a signed human decision. |
| INV-S7 | safety | When the fingerprints differ, `apply` is not called under any circumstance. |
| INV-L1 | liveness | Every candidate or verifying transformation reaches a terminal state or expires, in finite time. |
| INV-L2 | liveness | Every escalation resolves or expires; there is no indefinite hold. |
| INV-L3 | liveness | Crash recovery terminates and leaves nothing stuck in `Committing`. |
| INV-L4 | liveness | Waiting on a conflicting predecessor is bounded, because the time-to-live applies while waiting. |

### 2.8 Crash recovery

The journal is write-ahead. Every transition is recorded before its external side effect, so a crash inside the commit critical section is recoverable and not ambiguous.

On start-up the journal is replayed to the last record for each transformation. A transformation whose last record is terminal is restored without re-running anything. A transformation whose last record is `CommittingStarted` with no following terminal record is resolved by asking the ledger. If an entry exists, the commit completed before the crash, and only the receipt and journal entry need reconstructing from the existing inclusion proof. If it does not, the commit-time steps re-run from the fingerprint comparison onward, which is safe because `apply` is idempotent by adapter contract and `ledger.append` is key-idempotent.

The outcome is exactly-once commit from any crash point: at most one ledger entry per transformation, and at most one verifiable receipt set.

---

## §3 Receipts: what a third party can check without us

<!-- src: 42 §3.10 (DSSE envelope, two receipt kinds) / 42 §3.11 (tile log, RFC 6962 domain separation) / 42 §3.14 (VerdictCheckpoint, VerdictTally) / 42 §5 (scope of verification) / 34 AC-057 -->

### 3.1 Two kinds of receipt

Every verdict produces a receipt, not only the ones that admit. A **VerdictReceipt** is issued for `Admit`, `Deny`, and `Escalate` alike, carrying the verdict summary, the canonical transformation identifier, and the precondition fingerprint. A **CommitReceipt** is issued only when a commit succeeds, and additionally carries the ledger inclusion proof, the post-condition fingerprint, and the content identifier of the escrowed inverse.

The payload, in full:

| Field | Type | Notes |
|---|---|---|
| `receipt_kind` | `VerdictReceipt \| CommitReceipt` | determines which of the following are populated |
| `transformation` | `TransformationId` | the subject |
| `verdict` | summary + digest | the verdict's kind and the content identifier of its proof, not the proof itself — receipts stay small |
| `canonical_cid` | `Cid` | fixed at plan completion; re-confirmed at canonicalisation for a commit |
| `inclusion_proof` | `Option<InclusionProof>` | mandatory for a commit receipt, **always absent on a verdict receipt** — this absence is what §3.5 exists to compensate for |
| `precondition_fingerprint` | `Fingerprint` | the plan-time reading |
| `postcondition_fingerprint` | `Option<Fingerprint>` | after application; absent on a verdict receipt |
| `inverse_delta` | `Option<Cid>` | the escrowed inverse; absent when none could be constructed |
| `enforced` | `bool` | false when applied under record-only mode despite a denial |
| `key_id` | `KeyId` | matches the signature's key identifier |
| `issued_at` | `Timestamp` | **not part of the signed core** — see §4.2 |

Both kinds are DSSE envelopes: a payload type string, a payload of canonical DAG-CBOR bytes, and a list of Ed25519 signatures. What gets signed is the pre-authentication encoding of the payload type and the payload together, which makes the payload type part of what is signed and not a hint sitting beside it.

Four distinct payload types exist in the system, and confusing two of them is refused **in both directions**. A signed tree head does not verify as a count, and a count does not verify as a tree head. Testing both directions is not thoroughness for its own sake. This class of load is signed by the party who benefits from the smaller of the two readings.

### 3.2 Identity

Identity is a BLAKE3-256 digest over a canonical DAG-CBOR encoding, wrapped in a 32-byte newtype and rendered as `gx1:<base32>` wherever a human or a JSON document has to see it. The canonical encoding is deterministic by construction: shortest-form integers, string map keys in bytewise lexicographic order with no duplicates, no indefinite-length encodings, no floats in canonical structures, and no CBOR tags. Content identifiers are embedded as plain 32-byte byte strings and not as tagged links, which costs self-description and buys one fewer dependency and one fewer way to encode the same value.

Each persisted structure has an *identity view*: the subset of its fields that its content identifier is computed over. The general rules are that self-referential identifier fields are excluded, timestamps are excluded (when a thing was recorded is not part of what happened), lifecycle state is never encoded, signatures live outside the payload they sign, and measured quantities do not contribute to a transformation's identity. Identity is fixed in two stages: `IntentId` at submit, `TransformationId` at plan completion. Neither moves afterwards.

A JSON path exists in parallel — RFC 8785 canonical JSON with SHA-256 — for interoperation with in-toto and SCITT-shaped tooling. It is explicitly *not* an identity: the digest it produces has its own type and is never compared against a content identifier.

### 3.3 The log

Receipts are anchored in an append-only Merkle transparency log with a tile-backed layout following Rekor v2 and the tlog-tiles design [2°].

Leaf and interior hashes are domain-separated in the manner Certificate Transparency established [PB, RFC 6962 §2.1]:

```
leaf_hash = BLAKE3(0x00 || canonical_dagcbor(LedgerLeaf))
node_hash = BLAKE3(0x01 || left_hash || right_hash)
```

The prefix bytes are standard second-preimage protection. We adopt the construction and substitute the hash function, and we claim nothing about the substitution beyond that it is the same construction with a different primitive.

The log emits inclusion proofs, consistency proofs, and signed checkpoints (tree size, root hash, origin namespace, signature). Anchoring to an external transparency log such as Rekor is designed for and not shipped. The default is our own log only, which currently leaves the log's operator as its only witness. §4 says so.

### 3.4 Offline third-party verification

The receipt exists for one property: someone who does not trust the issuer, and cannot reach the issuer, can still check it.

That property is written as an end-to-end acceptance test in a network-severed environment. Export a committed transformation's receipt to a file, place it in a namespace with no network, run the verifier without contact with the server, the log, or the gate, and require that signature verification, inclusion-proof verification, and canonical-identifier consistency all pass. Then require that a receipt with a single bit flipped fails, in the same severed environment. Two cases, one asserting the positive and one asserting that the check is capable of failing, because a verifier that accepts everything passes the first case perfectly.

**What that verification covers, exactly.** It covers the meta-structure of the change: hash chains, signatures, ledger inclusion, identifier consistency. **It does not reproduce the data.** Object bytes are not stored, only their digests, so a verifier can establish that a transformation with these properties was admitted and committed, and cannot reconstruct what the file said. Customers who need reproduction need an evidence-retention configuration, which is a commercial feature and not part of the open core. The data-model specification carries this boundary in writing, so nobody has to discover it.

### 3.5 Counting the verdicts nobody exports

There is a hole that signing does not close, and the system's answer to it is the part of §3 we would most like attacked.

`VerdictReceipt.inclusion_proof` is fixed at `None` by construction, because verdicts are not entries in the ledger. Only commits are. So an operator who simply never exports a `Deny` receipt can show an auditor a ledger consisting entirely of admitted, committed changes, and **nothing in it contradicts anything**. The ledger is complete and honest about commits and silent about refusals, and silence is the shape the misconduct takes.

The answer is a **VerdictCheckpoint**: a separately signed, fourth-payload-type artifact carrying a **VerdictTally** and a window.

| Field | Type | Purpose |
|---|---|---|
| `origin` | string | log namespace, so one deployment's counts cannot be read as another's |
| `tally.deny` | u64 | count of `Deny` verdicts in the window |
| `tally.admit` | u64 | count of `Admit` verdicts **the gate actually produced** |
| `tally.escalate` | u64 | count of `Escalate` verdicts |
| `tally.unverdicted` | u64 | count of T-4e admissions — fail-open proceeded, the gate never ran |
| `window_start`, `window_end` | u64 | half-open interval over verdict sequence numbers; each checkpoint opens where the previous one closed |
| `ledger_root_hash` | Option\<Cid\> | head of the commit ledger at minting time; `None` when the window committed nothing |
| `ledger_tree_size` | u64 | leaf count at that head |
| `timestamp` | Timestamp | **outside the signed core** |
| `signature` | DsseSignature | ledger signing key |

Four design decisions in that table are load-bearing.

**Four buckets, not three.** `unverdicted` exists because folding T-4e into `admit` would report a judgement nobody made. The verdict enum still has three variants. T-4e is not a fourth kind of verdict, it is the absence of one, and that is exactly why the tally needs a fourth number.

**`ledger_root_hash` is optional, and that is not a degenerate case.** A window in which everything was denied commits nothing, so the tree is empty. That window is the reason the mechanism exists.

**`ledger_tree_size` is the anchor.** Every ledger leaf is a commit, and every commit is downstream of an admission. A checkpoint chain whose cumulative admissions fall short of the ledger's current leaf count has stopped being published. An operator cannot quietly shrink the ledger's size, which is what makes it worth binding to.

**The timestamp sits outside the signed core.** A verified checkpoint is a statement about counts. It says nothing about when it was written and does not pretend to, for the same reason the receipt's `issued_at` is unsigned (§4.2).

No hash chain links checkpoints. Tamper-evidence of a checkpoint's *contents* is already what the signature provides. What a `previous` link would add is protection against substituting a differently-signed checkpoint with the same boundaries, and that substitution requires the key, and an attacker holding the key can re-sign a chain of any length. So **window contiguity is the detector for a missing checkpoint**, and **`ledger_tree_size` is the detector for a truncated tail**.

Under-reporting is then detectable arithmetically, even when the lie is re-signed: a checkpoint claiming fewer than the verifier can count produces `ChainBreak::Underreported { kind, claimed, observed }`. The test that establishes this re-signs the lie, because a test that flips one bit is measuring signature integrity and calling it under-reporting detection.

**Two things it does not close, stated here and not in a limitations appendix:**

1. **Policy relaxation is not addressed.** A gate widened until it refuses nothing reports `deny = 0` truthfully. What the checkpoint supports is the detection of *non-disclosure*, not of permissiveness. A test in the repository is named for the fact that this detection does not fire, so that if it ever starts firing, someone finds out.
2. **Split view is not addressed.** A signature attests that this key stated these numbers. It does not attest that these are the only numbers this key stated. One key can sign two internally consistent chains for two verifiers. Detecting the fork requires verifiers to compare notes, which is a consistency-proof problem and is not solved here.

---

### 3.6 One tool call, end to end

The abstract shape is easier to check against a concrete trace. Suppose an agent, speaking MCP through the proxy, calls a tool that rewrites a configuration file.

1. **The call is intercepted, not forwarded.** The proxy turns it into an intent: substrate `Mcp`, a locator naming the resource, the tool arguments as the goal, and the calling actor's key. The intent is canonically encoded, and its content identifier is the `IntentId`. Nothing has been sent to the server.
2. **The adapter plans.** It snapshots the target, produces a `PlannedDelta` describing the call it *would* make, and computes `Fingerprint_0` over the relevant server state. `plan` is pure, and for the MCP adapter this is measured, with counters asserting zero tool calls and zero reads during planning. The transformation's canonical form is now complete and its identifier is fixed.
3. **The gate is asked.** It receives the transformation, the pre-state, the planned delta, the collected evidence, and one boolean: whether an inverse is available. Cedar policies and registered invariants are evaluated. Suppose the answer is `Admit`. A `VerdictReceipt` is signed and issued for it, and one would have been issued for `Deny` or `Escalate` too.
4. **Canonicalisation re-checks itself.** `canon(T)` is recomputed and compared against the identifier fixed in step 2. A mismatch here is not an abort. It is an internal error, because it means either a bug or tampering.
5. **The critical section opens**, journalled before anything external happens. `Fingerprint_1` is taken. If the server state moved since step 2, the call aborts with `PreconditionChanged` and the tool is never invoked.
6. **The inverse is escrowed.** The adapter constructs the reversing call and it is persisted in full. Only now —
7. **The tool call is finally made.** This is the first moment anything reaches the server.
8. **The ledger appends** the canonical transformation and its verdict, yielding an inclusion proof, and a `CommitReceipt` is signed over the whole set: verdict, canonical identifier, inclusion proof, both fingerprints, the escrowed inverse's identifier, and the enforcement flag.
9. **Undo, if it happens**, is a fresh intent whose goal is the escrowed inverse. It repeats steps 1 to 8 in full, gate included, and on committing draws a supersedes edge to the original, which is left exactly as it was.

The property the ordering buys is narrow, and worth stating precisely. **Between step 3 and step 7 there is a signed record of a decision about a change that has not happened yet, and between step 6 and step 7 there is a stored means of reversing a change that has not happened yet.** Neither is a guarantee about outcomes. Both are artifacts that exist before the outcome does.

The trace also shows the sharp edge from §4.2. At step 7 the server may do something the locator does not name, and steps 2, 5 and 8 all measure the object and not the server's whole state. The proxy's answer to that is not detection. It is serialisation, so that nothing else runs on that server beside this call.

---

## §4 Declared coverage: the boundary, published

<!-- src: 45 §2 TH-1..TH-12 / 45 §3.1 candidates 12,13,14 / 42 §3.15 / 21 §10-4 (/limits) / 51 §11.1 v0.2.3 note -->

### 4.1 Why this is a section and not an appendix

The most dangerous property a verification system has is that it makes a reader feel covered.

So the discipline this project adopted is that the boundary of coverage ships as an artifact. Not a caveat. A named list, published beside the mechanism, with a page in the documentation whose whole content is what the system does not do, one click from the top of the site.

The engineering form of the same discipline is that skips are named. A benchmark that cannot be judged because the filesystem is wrong prints `AC067_NOT_GATED` instead of passing quietly. A build without an MCP wire prints `MCP_IS_NOT_REGISTERED` instead of reporting a substrate it does not have. The CI script prints `SKIPPED` lines. A skip that looks like a pass is the failure mode all of this aims at, and we have shipped it wrong before: a benchmark-checking stage keyed on a filename that did not exist in the tree went green without ever finding a benchmark, and stayed green for as long as the specification carried the stale name.

### 4.2 The list

This is the content of the `/limits` page, in English. It is an extract of the threat model; it introduces no limits of its own.

| We do not claim | Why |
|---|---|
| **If the policy and the intent diverge, Tracefold enforces the divergent policy faithfully.** The admissibility predicate is written by the operator. A change that is malicious but predicate-satisfying is admitted, and a signed receipt is issued for it. | Oracle problem. Structural; not mitigated, only bounded — by verified undo, and by shipping default policy packs so that nobody starts from an empty predicate. |
| **Out-of-band writes by root or kernel-privileged actors are not detected.** Not prevented, not detected: not seen at all. | The adapter is the only sensor. Detection (not prevention) via Landlock and eBPF is roadmap work for v0.3+, and even then would be incomplete against an attacker with kernel privileges. |
| **Key revocation cannot cover the time the key was actually compromised — and a timestamping authority does not close this.** The retroaction range is a policy setting; what the machine checks is consistency with the setting, not correspondence to reality. | Compromise time is an event outside the membrane that nobody observed. The default setting trusts `Receipt.issued_at`, which is deliberately *outside* the signed core, so a holder of the compromised secret can re-issue the same payload with an earlier `issued_at`. A timestamping authority proves when a signature was made, not when a key leaked; a receipt signed between compromise and revocation is timestamped before revocation and passes. The alternative setting reads no clock at all and pays for it in availability, invalidating legitimate pre-revocation receipts too. |
| **We cannot observe an MCP tool call whose effect lands outside the object the transformation is about.** | The protocol does not tell a proxy what a tool touches. This is a gap in MCP, not an unimplemented feature: the fingerprint reads the object, and the object is not where the effect went. What we offer instead is **serialisation** — the footprint is the server, so nothing else on that server runs beside it. The price is that two resources on one server conflict. |
| **Lexical normalisation of a locator does not cover alias resolution inside the substrate.** | All three substrates resolve names outside our lexical layer: filesystem symlinks, Git reference resolution, and MCP servers that map one URI to another resource. We can say our normalisation of *the string we named* is deterministic. Whether that string resolves where you think it does is the substrate's property. |
| **A count checkpoint closes non-disclosure only.** Relax the policy until everything is admitted and the counts are zero and legitimate. Show two verifiers two different chains and both are internally consistent. | §3.5, limits 1 and 2. |
| **Continuous differential testing against a formal (Lean) model is planned work for milestone M8. There is none today.** | No `lean/` directory, no conformance corpus, no artifacts for the three acceptance criteria that would cover it. See §7.4. |
| **Machine-enforced CI on push and pull request covers 2 of 13 crates.** The rest of the green is a record of a human running the full script. | §5.5. |

### 4.3 Where the boundary came from

The twelve adversary entries the boundary is derived from, with the residual risk each carries after mitigation. "High" here means we do not believe the mitigation closes it.

| # | Threat | Mitigation, if any | Residual |
|---|---|---|---|
| 1 | A compromised agent submits a change that satisfies the policy and is nonetheless harmful | none possible; verified undo bounds the damage; shipped policy packs reduce the empty-predicate problem | **high, structural** |
| 2 | An actor with shell or root access writes outside the adapter | none in the current version; detection (not prevention) is roadmap | **high** |
| 3 | The precondition changes between verification and application | fingerprint compare-and-swap at commit time; abort rather than apply | medium → low; larger for non-atomic adapters |
| 4 | A policy change is submitted as an ordinary change, weakening the judge | policy edits are themselves gated as order-2 transformations | **high** — the "strictly attenuating" property that would make this safe is research, not shipped; the current answer is multi-party approval as an operational practice |
| 5 | An actor key or the ledger signing key is compromised | generational key identifiers and rotation; revocation ledger | medium — no timestamping authority, so the revocation time carries no third-party proof |
| 6 | The ledger operator rewrites or rolls back history | append-only tile log with inclusion and consistency proofs | medium — external anchoring is designed and not shipped, so the log's operator is currently its only witness |
| 7 | The same intent is replayed to double-apply | identity is the content identifier of the canonical form, so a replay is the same transformation; apply is idempotent | low |
| 8 | An evidence collector fabricates results | evidence is signed and unsigned evidence is not accepted | medium to high — a compromised collector signs its fabrication correctly; the gate checks authenticity, never truth |
| 9 | The verifier is attacked to stop work, exploiting fail-closed | record-only mode as an availability escape hatch, stamped `enforced=false` | medium — choosing fail-closed is choosing to bet availability on the verifier, and record-only preserves availability by discarding the gate's meaning |
| 10 | A dependency or toolchain is compromised | dependency set restricted to a reviewed table, lockfile pinning, toolchain pinning, licence gate | medium |
| 11 | A receipt leaks confidential information through the locator | object bytes are never stored; the locator string is not redacted | medium — the locator is in the clear today |
| 12 | An undo is itself harmful | the undo goes through the whole pipeline; commutation detection escalates on conflict with later changes | medium — the undo inherits threat 1 in miniature |

Three further items were added in 2026-08, after a sweep whose explicit purpose was to find limits the membrane had acquired without declaring them. The sweep's own completion condition was that new limits must be added *with names* and not absorbed. Two of the three are in §4.2, revocation versus compromise time and effects landing outside the object. The third, locator aliasing, turned out to be a pre-existing gap and not a new one, and was filed as such instead of quietly excluded.

Read row 1 and row 4 against each other. They are the same shape: a predicate that does not say what its author meant, and a change that edits the predicate. Row 4 is the sharper one, because at the level of the pipeline the operation that weakens the judge looks like any other order-2 change. Closing it would take a decidable requirement that a policy edit only ever shrink the set of admissible changes. That is a research item, and this report does not claim it.

One consequence of the discipline cuts against us and is worth stating. The list gets longer over time, and it gets longer fastest when the system gains surface area. A reader who compares this report to a competitor's material and finds ours has more admitted limits should not conclude that ours has more limits.

### 4.4 The two settings that decide how much the gate means

Coverage is partly a deployment choice and not only a property of the mechanism. The two axes are independent.

| Setting | Values | Effect |
|---|---|---|
| **Fail posture** — what happens when the verifier or evidence collector cannot be reached | `FailClosed` (default, all substrates) / `FailOpen` (per-substrate, explicit opt-in only) | Fail-closed aborts with `VerifierUnavailable`. Fail-open proceeds for that one transformation, degraded to record-only, and stamps the receipt `enforced=false` and `fail_posture_engaged=true`. |
| **Enforcement mode** — what happens when the gate says no | `Enforce` (default) / `RecordOnly` (per substrate or globally) | Under enforce, `Denied` is terminal. Under record-only, a denied change is applied anyway and its receipt is stamped `enforced=false`, so that "this was applied and the policy had refused it" is a third-party-checkable fact rather than an absence. |

Neither setting is a hidden default and neither is silent. Both leave a mark inside the signed structure, and the count checkpoint keeps them separable, since a fail-open proceed lands in `unverdicted` and not in `admit`. The symmetry extends to undo: under record-only mode an undo request is honoured the same way a commit is, applied and stamped `enforced=false`, so switching the mode on does not quietly remove the ability to reverse what it let through. <!-- src: 32 §K FR-M7-1 -->

The trade-off is real in both directions and we don't pretend otherwise. Choosing fail-closed makes the verifier a dependency of the business operation, and an attacker who can stop the verifier can stop the work. Choosing record-only preserves availability by keeping the record and discarding the enforcement, which is a coherent position for an organisation measuring how much a policy *would* have refused before turning it on. It is also not the same product as the one the rest of this report describes.

---

## §5 Measurements

<!-- src: req/38 §65 (floor 1370/247, tag req0.09) / §62 R-1 (AC-067 conditional) / §62 (FR-M7-2 two-arm) / 51 §9 / 51 §11.1 v0.2.3 / 33 ASM-33-1 / 34 AC-067 -->

### 5.1 How to read the numbers

Read every number below under three rules.

1. **Every number carries its conditions in the same sentence.** A latency figure without its filesystem is not a figure.
2. **The performance thresholds in the specification are provisional design budgets, not pass lines.** The non-functional requirements NFR-001 to NFR-004 are marked provisional pending an owner decision informed by measurement. Until then, "2.6–6.5% of the budget" is a true statement and "meets the requirement" is not one we make.
3. **We report what was excluded.** Where a measurement was not gated, or not wired, or measured in-process instead of through the real surface, that is written down beside it.

All figures come from a single developer machine: WSL2 on Windows, single node, single process, under an advisory lock that refuses to run beside a second measurement.

### 5.2 Test floor

| Quantity | Value | Conditions |
|---|---|---|
| Test probes | **1,370** | workspace-wide, at commit `b407365` |
| Test suites | **247** | same commit |
| Independent agreement | 4 parties | declared count, reconstructed count, actual run, and an independent re-run by a separate adjudicating lane, all agreeing |
| State-machine transition coverage | **21 / 21** | tracked in a transition-to-test table, checked by a dedicated CI stage |
| Growth over milestone M7 | 968 / 170 → 1,370 / 247 | start to end of the milestone |

**A disclosure about the floor.** As of the most recent audited commit (`c7cdb6a`), one probe in that floor is **red**, and it is red on purpose.

The probe asserts that a particular journal record's shape follows the state-machine document instead of an older data-model document, and its own comment says why it exists: *so that an erratum landing in the data model breaks a probe rather than leaving a divergence nobody re-reads*. A specification revision landed exactly that erratum. The probe did its job. It is scheduled to be updated to assert the new agreement, with the old expectation retained in a comment.

We report this because the alternative — quoting 1,370 green and letting a reader discover the red — is the behaviour §4 exists to prevent. The measurement discipline the incident produced is also new and worth stating: **a documentation-only revision can break the test floor, because tests that read documentation exist.**

### 5.3 MCP proxy overhead

The question is what a proxy that holds and gates tool calls costs when it is *not* gating. That is the passthrough path, and it is the path most calls take.

> On a configuration where the journal and ledger are on **tmpfs**, the proxy's added latency is **2.6–6.5% of the 30 ms design budget** — median **1.276 ms** across four runs, and **795 µs** on a long run at n = 3,000.
>
> On **ext4 (a WSL2 virtual disk)**, p99 is **163.8 ms**, which overruns the budget by 5.5×.

The second number is the informative one, and it is not a number about the proxy's code. It is a number about **fsync: 15.1 calls at a measured unit cost of 5.9 ms**, whose product (89 ms) accounts for the measured 93 ms within the resolution of the instrument. The call count came from `strace`, at 302 calls over 20 operations.

Reading the ext4 figure as "the proxy is slow" is a misreading. Reading the tmpfs figure without its filesystem is the same misreading pointed the other way. Reducing the 15.1 fsyncs, through group commit or journal batch fsync, is registered as a v0.2 design item.

The acceptance criterion for this measurement was originally written without naming a filesystem, while the neighbouring criteria for the commit pipeline *do* name tmpfs. That omission was a choice at drafting time and not an oversight, and it was corrected: the criterion is now conditional, and on a filesystem where the condition does not hold, the benchmark prints `AC067_NOT_GATED` with a reason instead of judging.

### 5.4 Inclusion-proof construction

Rebuilding an inclusion proof from cached tile hashes instead of walking the leaves:

| Quantity | Value |
|---|---|
| Speed-up | **131.8×** at n = 64,000 |
| Median, before | 10.827 ms |
| Median, after | 82.163 µs |
| Two-arm ratio test | fail arm 8.855, pass arm 1.394 (a declared ratio threshold, red before green) |
| Wire format | **unchanged — the proof fingerprints of both arms are identical** (`gx1:nj4nv7b4…`, 147 proofs) |

The last row matters most, and it is a measurement rather than an assurance: the faster path emits byte-identical proofs, so the third-party verifier is untouched. A second detail from the same run is more diagnostic than the headline. Monotonicity in n disappeared, with a run at n = 8,000 coming out faster than one at n = 1,000, and that is the actual evidence that the linear term is gone.

The oracle used to check the new implementation was written as an independent transcription of RFC 6962 §2.1.1, without reading the crate's code. Checking a new implementation against a re-import of the old one produces green by shared mistake.

### 5.5 What is machine-enforced, and what is not

This is the measurement we expect to be least comfortable and are most concerned to publish.

| | Scope that runs | Started by |
|---|---|---|
| `tools/ci.sh` (manual) | all stages, all 13 crates | a person |
| `.github/workflows/ci.yml` (push / PR) | **2 crates** (`gx-core`, `gx-canon`) for the format, lint, and test stages | GitHub Actions |

The reason is a workspace member with a path dependency on a tree that does not exist on a runner holding only this repository. The manifest cannot be resolved, so the workflow drops that member from the checkout and narrows the scope, and the CI script prints the narrowing as `SKIPPED` lines instead of letting a narrowed run look like a full one.

**The consequence is stated plainly: of the gates this report describes, the ones that can turn a pull request red automatically cover 2 of 13 crates. The remaining green is a record of a human having run the script.** Widening the scope is registered work, not done.

Related disclosures in the same family:

- Five verification layers are opt-in and default off: Kani model checking, fuzz regression, coverage thresholds, benchmarks, and mutation testing. They are named stages with names printed when skipped, not silent conditionals.
- Benchmarks are deliberately kept out of the deterministic gate, because they are noisy. Three exceptions judge by exit code, because each declares its threshold before the numbers exist, and each prints a `BUDGET_SOURCE` beside the figure so that a loosened run cannot be read as a run under the declared budget.
- Two of the nine benchmarks are not wired into the CI script at all. They print medians and denominators and have no thresholds; wiring them would not create a gate. This is a disclosure, not a change.

The nine, and what each one's number is allowed to mean:

| Benchmark | Crate | Standing |
|---|---|---|
| `verify_latency` | `gx-gate` | printed against a provisional 50 ms p99 budget; does not fail the build |
| `commit_pipeline` | `gx-engine` | printed against a provisional 250 ms p99 budget on tmpfs; does not fail the build |
| `throughput` | `gx-engine` | printed against a provisional 100 commits/s budget; does not fail the build |
| `journal_recovery` | `gx-engine` | printed against a provisional 5 s recovery objective for a 10,000-entry journal; does not fail the build |
| `proxy_latency` | `gx-adapter-mcp` | **judges by exit code**, and only when the journal is on tmpfs; otherwise prints `AC067_NOT_GATED` with the reason (§5.3) |
| `inclusion_proof` | `gx-log` | **judges by exit code** against a ratio threshold declared before the numbers existed (§5.4) |
| `disjoint_lock` | `gx-engine` | **judges by exit code** against a regression floor set 20× below the observed value — a threshold chosen so a noisy benchmark can go red safely, which is a regression gate and not a claim about concurrency |
| `serve_throughput` | `gx-api` | no threshold; prints median, count, and denominator, plus the budget beside them explicitly *not* compared. **Not wired into CI.** It also discloses that it does not open a socket. |
| `recover_cost` | `gx-engine` | no threshold; prints median, count, and denominator over a real journal of roughly 10,000 records with 101 unresolved commits. **Not wired into CI.** |

The pattern in the right-hand column is deliberate. A benchmark may fail the build only if it declared its own threshold before anyone had measured the quantity. A benchmark aimed at a number the specification calls provisional prints and does not judge, because failing against an undecided number teaches people to raise the number.

### 5.6 Distribution

The CLI and API build as a single statically linked binary. `readelf -d` reports **zero `NEEDED` entries**, there is **no `INTERP` segment**, and the artifact starts and reports its version. No container and no daemon is required to run it. We take the answer from the ELF itself and not from `ldd`, because `ldd` on a static binary says different things on different systems.

---

## §6 Related work: where each part came from

<!-- src: 42 §3.11 (RFC 6962), 42 §4 (in-toto/SCITT correspondence), 41 §2 (dependency choices), 12 (formal-semantics lineage), req/108 D-1..D-5 (agent-side survey, gh-verified 2026-08-13), 21 §3 -->

§1 makes a claim about assembly, and an assembly claim is only checkable if the parts are named. That is what this section is for. Strength labels are as declared at the top of this report.

### 6.1 Transparency logs and tamper-evident logging

Certificate Transparency fixes the leaf and interior domain separation we use unchanged, substituting our hash function [PB, RFC 6962]. The tile-backed storage layout, and the decision to run a transparency log without a separate database tier, follow Rekor v2 and the tlog-tiles design [2°]. Signed checkpoints, inclusion proofs, and consistency proofs are that lineage's standard apparatus, and we add nothing to it. Earlier antecedents in tamper-evident logging (Schneier–Kelsey 1998; Crosby–Wallach 2009) are the acknowledged ancestors of the construction [2° for this draft].

**What is ours:** nothing in the log itself. What differs is what goes in it, canonical transformation records and verdict digests instead of certificates or artifact signatures, plus the separately-signed count commitment beside it (§3.5). That count commitment exists because our leaves are commits, which makes our refusals invisible to the tree by construction.

### 6.2 Signed attestation envelopes and supply-chain provenance

Receipts are DSSE envelopes, so a receipt is a valid in-toto attestation with a custom predicate type, and our data model documents the correspondence field by field [PB, in-toto Attestation Framework; DSSE specification]. in-toto reached CNCF graduated status, and this project explicitly treats it as an ally and not a competitor. The intended relationship is to be readable by that ecosystem's tooling.

SCITT is the closer fit conceptually, a transparency service whose receipt is a proof of registration, and the deliberate mismatch is that SCITT requires COSE_Sign1 while we ship DSSE. A conversion layer is designed and not implemented [PB, IETF SCITT architecture draft].

**What is ours:** the payload contents, the two receipt kinds (verdict versus commit), and the four-way payload-type separation tested in both directions. The envelope, the pre-authentication encoding, and the ecosystem are not.

### 6.3 Content addressing and canonical encoding

Content addressing over a deterministic encoding is Git's and IPFS's lineage. Our specific encoding is IPLD DAG-CBOR with RFC 8949 core deterministic rules, minus CBOR tags [PB]. The parallel JSON path is RFC 8785 JCS [PB]. The text form of an identifier is RFC 4648 base32 [PB]. The hash function is BLAKE3 [2°].

Our departures are small, and each is an engineering choice with a cost. We do not use CIDv1's multicodec self-description, which buys one less dependency and one less way to write the same value and costs the self-description. We forbid tags, the IPLD link tag included. And we exclude floats from canonical structures entirely, which is why measured quantities are referenced by digest instead of embedded.

### 6.4 Compensating transactions

Pre-provisioning an inverse, and treating undo as forward motion instead of as rollback, is the compensating-transaction pattern, whose canonical statement is the Sagas work [PB, Garcia-Molina and Salem, SIGMOD 1987]. Long-running workflow engines in current use, Temporal and Restate and Inngest among them, carry compensation as a first-class concept [2°].

**What differs is ordering, not concept.** In the usual formulation the compensating action is *specified* in advance and *constructed* when needed. Here the adapter constructs the inverse, the system checks it for constructibility, and it is persisted **before** the forward action runs, so its unavailability is an input to the admission decision and not a discovery made during recovery. Our internal survey of that space records that the workflow engines have compensation but not pre-execution escrow, not advance verification of the inverse, and not an offline-checkable record [survey 2026-08, method and scope in Appendix C].

### 6.5 Plan-then-apply

Deciding against a proposed change and then applying exactly the thing that was decided about is `terraform plan` / `terraform apply` [PB, HashiCorp documentation], and the same shape recurs in Kubernetes admission control and in database migration tooling. Terraform's `-out` plan file plus refresh-and-diff on apply is a close relative of our `PlannedDelta` plus commit-time fingerprint comparison.

**What differs:** the plan artifact here is content-addressed and is the subject of the signed record, the comparison at apply time aborts instead of re-planning, and the decision itself produces a receipt whether it admitted or refused.

### 6.6 Policy engines and their verification

The gate is Cedar, used as a library and not modified [PB, Cedar]. The differential-testing methodology we intend to use for the Lean model is the one the Cedar project itself uses between its Rust implementation and its formal model. That is the named precedent for the approach, and the approach is not running here yet (§7.4).

### 6.7 Concurrency control

Comparing a precondition fingerprint at verify time and again at commit time, and aborting on mismatch, is optimistic concurrency control with compare-and-swap. The choice is deliberate against a global lock, whose cost would be exactly the adoption objection this system is most vulnerable to.

The independence test between two pending deltas (`Commutes` / `Conflicts { residual }`) is grounded in parallel independence from double-pushout graph rewriting [2°], and the residual is the obstruction. Our internal formal-semantics document is explicit that this is a *generalisation* of a commutator to substrates without a group structure, and that calling it a commutator would be a type error in the general case.

### 6.8 The semantic layer, and what is deliberately not built on it

The internal specification carries a layered formalisation. It is described here and not in §2 for one reason: only the bottom layer is shipped. The project's own rule is that citing the upper layers as implemented is a violation, which leaves listing the ancestors as the honest form of the section.

**The shipped layer.** Admissible morphisms as a multiplicative morphism property, closed under composition and containing identities, which is a wide subcategory. Invariants as an indexed family with a sequential composition rule, which is Hoare logic in a categorical dress [PB]. Witnesses as a lax functor into a monoidal category of evidence, so that the witness of a composite is bounded by the composition of the parts' witnesses. Measured quantities as a lax functor into a quantale, following Lawvere-style enrichment [2°], with an optional Lyapunov-shaped law bounding the measure of the output by the measure of the input plus the cost of the morphism.

Five statements are named as the things a formal model would be asked to prove: composition preserves admissibility; Hoare triples compose; canonicalisation is idempotent and representation-independent; receipt verification implies both admissibility and ledger inclusion; and the verdict chain of a composite is recoverable from its receipts. **None of the five has been mechanically proven. They are the M8 assignment (§7.4).** They are listed because a reader should be able to see what the eventual proof obligation is, and because the properties are stated as testable requirements in the meantime.

**The layer above, not shipped.** Change actions — a monoid of changes per object with an application operator, and derivatives satisfying `f(x ⊕ δ) = f(x) ⊕ f'(x, δ)` — are the intended semantics for indexing change by something other than time. The lineage is incremental lambda calculus (Cai and colleagues, 2014) and its later categorical treatment (Alvarez-Picallo and Ong, 2019) [PB], with DBSP as the worked database instance [2°].

This matters to one specific honesty point. The founding intuition of the project wrote interference between two changes as a commutator, a difference of two composites. **That subtraction is a type error in a general category**, and it is definable only where the change structure is an abelian group. Where it is not, on filesystems for instance, the correct generalisation is an independence judgement with a residual, grounded in parallel independence in double-pushout graph rewriting [2°]. The specification records this as a downgrade of the original claim, not as a design choice discovered later.

**The research layer, not shipped and not claimed.** Double categories, confluence of canonicalisation via critical-pair analysis and Newman's lemma [PB], and the connection between a mechanically verified admissibility theory and a cryptographic witness. That last item is where the project puts whatever novelty it might eventually have, and its current standing is a research question with no result. This report makes no claim about it, and the internal document that defines it carries an explicit prohibition on describing it as done.

Proof-carrying code is the acknowledged relative of the whole shape, shipping the artifact with the evidence that it satisfies the policy, and provenance semirings (Green, Karvounarakis and Tannen, 2007) [PB] are the acknowledged relative of the evidence algebra. Fork consistency (Mazières and Shasha, 2002; SUNDR, 2004) [PB] is the lineage in which §3.5's split-view limitation is a known degenerate case. We claim nothing new there and say so in the limitation itself.

### 6.9 The agent-side neighbourhood

The following was surveyed on 2026-08-13 by directed search followed by primary verification of repository, licence, and star count through the GitHub API. Star counts move. Licences, and the presence or absence of a mechanism, are the durable part.

| Project | Licence / scale (2026-08-13) | What it does in this space |
|---|---|---|
| aider | Apache-2.0, ~48.1k stars | Git auto-commit at conversational-turn granularity; undo is `git reset`. File edits only; shell side effects are outside it. The turn-granularity commit is a UX standard we consider worth matching. |
| goose | Apache-2.0, ~52.7k stars | MCP-native agent; rollback depends on Git. |
| OpenHands | MIT, ~83k stars | Event stream of action-observation pairs; no native restore mechanism independent of Git. |
| opencode | MIT, very large | Session snapshots; shell side effects may not be restored. |
| Open Interpreter | Apache-2.0 | `%undo` reverts conversation history, not files. |
| Claude Code | proprietary | Local filesystem snapshots per conversational turn; changes made through the shell, and manual edits, are outside them. |
| docker/mcp-gateway | MIT, ~1.5k stars | Container isolation closes network-level bypass; policy at the lifecycle and credential-injection level; no tool-argument inspection. |
| mcp-scan (Invariant Labs) | Apache-2.0, ~2.9k stars | Tool-poisoning and rug-pull detection by hash change, prompt-injection scanning. As a proxy it can be bypassed by talking to the server directly. |
| hermes-agent | MIT, very large | Skill-origin provenance in a local database; self-reported, unsigned, no hash chain. |
| sb-runtime | Apache-2.0, 3 stars, last pushed 2026-04 | The closest published shape to ours: Cedar policy evaluated pre-execution, Ed25519-signed receipts with a parent-hash chain, RFC 8785 canonicalisation, offline third-party verification without host infrastructure, Landlock and seccomp isolation, single Rust binary. |
| punkgo-kernel | MIT, 4 stars | Append-only Merkle event log with checkpoints and a verification CLI. |
| agent-sign | Apache-2.0, 26 stars | Sigstore keyless signing with Rekor for agent actions. |
| BoundaryAttest / agent-proof-kit / AI-Proof-of-Us | MIT, 9 / 0 / 3 stars | Attestation of trust-boundary-crossing actions; in-toto-shaped; on-chain variants. |

Two readings of that table point in opposite directions, which is why both are here.

**Against us:** sb-runtime reached several of our central design decisions independently. Pre-execution evaluation, offline verification without the issuer, signed chains, a single static binary. That is evidence these decisions are the reasonable ones and not the insightful ones. Where we differ is the substrate adapter layer, escrow and inverse verification, the count checkpoint, the coverage declaration, and a self-contained deterministic encoding instead of JCS.

**For us, weakly:** within what we searched, on that date, by that method, no project in the signed-agent-receipt space has meaningful adoption. The largest is 26 stars and the nearest in shape has 3 and is stale.

We record what that does and does not license. It does **not** license a claim that demand exists. Adoption is not measured by the absence of competitors, and the project's own rule is that a market is validated by talking to buyers and not by counting repositories. What it licenses is narrow: nobody can give "an entrenched winner already owns this" as a reason to stop, because on this evidence there is no entrenched winner.

### 6.10 The four things we looked for and did not find

Stated with the three-part qualification a negative claim requires.

> **As of 2026-08-13, within the set of agent execution environments, MCP gateways, and agent-receipt projects we searched — by directed search followed by primary verification of the repository, licence, and README of each candidate, with the survey and its denominators recorded in an internal ledger — we did not find a project carrying any of the following:**
>
> 1. **Escrow of an inverse before execution, with the inverse verified in advance.** The undo layer in this space is at the level of file snapshots and Git commits.
> 2. **Observation of what a tool call actually touched.** *We do not have this either.* The MCP protocol does not report what a tool touches, so no proxy can. What we do instead is serialise per server. This item is in the list because it is a gap in the field, and its being a gap in the field is why we can be honest that it is also a gap in us.
> 3. **Receipts a third party can re-verify offline without the issuer.** The nearest exception, sb-runtime, does have this (§6.9) — which is why this item is a statement about the field's distribution and not about a single repository.
> 4. **A published declaration of what the system does not cover.**
>
> A negative result over a searched set is not a negative result over the world. If you know of a counter-example, it is more useful to us than a citation.

---

## §7 Non-claims

<!-- src: 45 §4.1 (six forbidden sentences) / 45 §4.2 (permitted sentences, v0.2.3 correction) / 31 §7 v0.2.3 / 51 §15 M8 / 21 §10-5 -->

The specification this system is built from carries a list of sentences that may not be used in documentation, marketing, or conversation. The grounding is that overstating belongs to the same category of failure as the threats the model enumerates. Those sentences, restated here as the things this report does not assert:

### 7.1 We do not claim the change is correct

The system evaluates a predicate the operator wrote. **It does not establish that a transformation is semantically correct, that it matches anyone's intent, or that it is safe.** What it establishes is that a transformation satisfied an agreed predicate, that the satisfaction is recorded in a form a third party can re-check, and that where an inverse was constructible it was verified and escrowed before the change was applied.

### 7.2 We do not claim accidents stop

Adopting this system does not mean an agent will not cause damage. The oracle problem (§1.3, §4.2) is not mitigated and is not mitigable. Verified undo bounds the consequences of a mistaken admission; it does not prevent one.

An undo can itself do harm. Reverting a change may destroy a later change that depended on it, which is why an undo goes through the whole pipeline including the commutation check, and why the same oracle problem applies to it recursively.

### 7.3 We do not claim to capture or prevent all writes

Everything in this report is scoped to changes that go through an adapter. A write that does not is not merely unprevented; it is **unseen**. Detection of out-of-band writes via Landlock and eBPF is roadmap work, is detection and not prevention, and would remain incomplete against a kernel-privileged adversary. There is no formulation under which we claim to close all paths.

### 7.4 We do not claim any formal-methods result, in any tense but the future

There is no Lean model in the tree. There is no differential-test corpus. There are no artifacts for the three acceptance criteria that would cover them, and the code generator that would produce the vectors does not exist. **Continuous differential testing between the Rust implementation and an independently formalised model is planned work for milestone M8, and its current progress is zero.**

We are explicit about this because the project got it wrong in its own documents and corrected it in the current specification revision. The forbidden-sentence list already prohibited *"the Lean proof and the Rust implementation have been proven mathematically equivalent."* But the sentence written in its place — *"verified by continuous differential testing"* — was in the present tense while the differential testing did not exist, and had become a way of asserting the forbidden thing through the exit. A comparison table in the product requirements likewise carried a filled-in mark for that property. Both were changed to a reservation, and the previous wording is retained in comments rather than deleted.

The condition under which the present tense becomes permitted is fixed in advance rather than left to judgement: when the release gates named `lean-current` (build succeeds, zero `sorry`, the five theorems reachable) and `difftest-nightly` (at least 10^5 vectors) are actually green as release blockers. Until then the future tense is the correct tense.

Related, and in the same family: we do not claim that the commutation mechanism is *proven categorically* or *verified with double categories*. The category-theoretic framing is a research layer explicitly excluded from the shipped product, and the internal document that defines it also contains the prohibition on citing it as implemented.

### 7.5 We do not claim the measurements are pass marks

The performance figures in §5 are measurements against provisional design budgets. Until those budgets are decided against real measurement, the correct sentence is "2.6–6.5% of budget on tmpfs" and the incorrect sentence is "meets the performance requirement." Two of nine benchmarks are unwired; five verification layers are opt-in; machine-enforced CI covers 2 of 13 crates.

### 7.6 We do not claim a single-writer deployment is a multi-writer one

Every measurement in this report is single-node and single-process. Throughput through the actual HTTP surface has not been measured concurrently, the in-process number is not a substitute, and it is registered as such. Multi-tenant behaviour, key management at organisational scale, and long-horizon retention are commercial-tier concerns that are designed for and not shipped.

### 7.7 We do not claim the name

Tracefold is provisional pending a registrar and trademark check, and an adjacent product in the `trace-*` naming neighbourhood was detected during the same survey that produced §6.9. If the check goes against us the name changes.

---

## Appendix A — Disclosure of AI use

<!-- src: methodology imported as a form from the project's earlier report §15; content is specific to this system -->

The sole author is a person. No tool is listed as an author or treated as one.

**How the work was organised.** The project runs as separated lanes, most of them large language models under human instruction, each lane holding one role:

- a **drafting lane**, producing specification text, implementation code, and prose including drafts of this report;
- **implementation lanes**, one per milestone hand, each with a written assignment and a prohibition on exceeding it;
- **adversarial audit lanes**, instructed to break the document or the code in front of them and to report blocking findings;
- an **adjudication lane**, which rules on each finding under a numbered decision and which performs an independent re-run of each milestone's frozen measurement script before accepting it;
- **research lanes**, used for breadth-first survey of prior work, whose output was not accepted without a separate party fetching the primary record.

**Vendor and models.** All lanes ran on Anthropic's Claude models through a command-line agent harness. Identifiers are given at family and class granularity, because writing a point version from memory would be the same failure this report is partly about.

**What the separation is for, and the evidence that it is not free.** The adjudication lane re-runs measurements instead of reading reports, because a lane reporting on itself is a lane grading itself.

The re-runs have found things. A milestone's floor was confirmed by four independent counts agreeing. A red probe was found by the adjudicating lane and not by the lane that landed the change (§5.2). A benchmark-checking stage was found to be matching a filename by substring, so a benchmark renamed with a suffix still satisfied it, and a stage that ran nothing reported success. Several instrument self-corrections are on record where the *measuring* script, and not the subject, was wrong, including one where a lane's own honest disclosure of a difference polluted the naive text search that was supposed to detect that class of difference.

**What no tool did.** No claim in this report rests on a tool's assertion about an external fact. The one external citation (§1.1) was fetched and read during drafting. Every irreversible decision, publication and licensing and naming and the scope of what is open, is the author's.

**Why this is a section rather than a line in an acknowledgement.** The subject of this report is machinery for making an agent's changes checkable by someone who does not trust the agent. A report about that, written by agents, with no statement of where the human gates were, would be self-refuting.

---

## Appendix B — Glossary

<!-- src: 11 §4 (canonical vocabulary) / 42 / 43 -->

| Term | Meaning here |
|---|---|
| **Object** | A content-addressed snapshot reference: substrate, locator, digest, representation kind. Bytes are not stored. |
| **Transformation** | A first-class value describing a change. Carries intent, subject, delta, context, actor, parents. Carries no lifecycle state. |
| **Order** | 0 for a change to an object, 1 for a change to a transformation, 2 for a change to that. Capped at 2 in the shipped implementation. |
| **Admissibility** | The predicate a transformation must satisfy: a Cedar policy set plus invariant checks plus evidence requirements. Written by the operator. |
| **Candidate** | Any unapproved transformation (broad sense); also the specific state after plan and before verification (narrow sense). Both usages are canonical and the ambiguity is documented rather than resolved. |
| **Gate** | The component evaluating admissibility. Returns `Admit`, `Deny`, or `Escalate`. |
| **Verdict** | The gate's answer. Three variants. A fail-open proceed is not a fourth variant — it is the absence of a verdict. |
| **Escalate** | A verdict routing to a human, resolved only by a signed decision, never automatically. |
| **Evidence** | Concrete grounds gathered at verification: test results, measurements, external attestations, policy evaluations. Signed; the gate checks its authenticity, not its truth. |
| **PlannedDelta** | The adapter's opaque description of a change, canonically encoded and content-addressed. The core treats it as bytes. |
| **Fingerprint** | An adapter-computed digest of the substrate state relevant to a transformation, taken at plan time and again at commit time. Its scope may be wider than the object itself. |
| **CAS** | Compare-and-swap on the fingerprint: mismatch aborts, and `apply` is not called. |
| **EscrowedInverse** | The inverse delta, persisted in full before the forward change is applied. May be `Unavailable` when the adapter cannot construct one. |
| **Commutation** | An adapter's judgement of whether two pending deltas are independent: `Commutes`, or `Conflicts { residual }`. |
| **Canonicalization** | The idempotent mapping to a representation-independent canonical form; the precondition for content addressing. |
| **Receipt** | A DSSE envelope over a canonical payload. `VerdictReceipt` for every verdict; `CommitReceipt` for successful commits, additionally carrying an inclusion proof. |
| **Ledger** | The append-only Merkle transparency log. Its leaves are commits. |
| **Checkpoint** | A signed tree head over the ledger. |
| **VerdictCheckpoint** | A separately signed commitment to verdict *counts* over a contiguous window, bound to the ledger by its tree size. Distinct from a checkpoint. |
| **unverdicted** | The count of admissions produced by fail-open when the gate did not run. |
| **enforced** | A receipt flag, false when a change was applied under record-only mode despite a `Deny`. |
| **fail_posture_engaged** | A receipt flag, true when fail-open proceeded because the verifier was unreachable. |
| **Superseded** | The terminal status of a committed transformation whose inverse has since been committed. Appended metadata; the original record is untouched. |
| **Substrate** | A system in which change happens: filesystem, Git, MCP server. Reached only through an adapter. |
| **Surface** | A shipping crate that depends on the engine. Subject to rule one and rule two (§2.4). |
| **Declared coverage** | The published boundary of what the system observes and asserts (§4). |

---

## Appendix C — What this draft has not checked

<!-- src: methodology (do-not-assert list) imported as a form; contents specific to this draft. Written last, on purpose. -->

A report whose do-not-assert list is empty is a report that has not distinguished what it checked from what it did not.

### C.1 Sources actually opened while writing this draft

Two, and both are named:

1. The AI Incident Database record for Incident 1152, opened 2026-08-13: title, incident date, and the list of five press reports it indexes were read. **The five reports themselves were not opened.** The incident narrative in §1.1 therefore rests on the aggregator's summary, which is why it is labelled [2°] and not [PV].
2. The repository itself, at commit `ebbf93d`, read through Git rather than the working tree so that a concurrently editing lane could not shift the ground under the citation. Every internal fact in this report is sourced to a document or file at that commit, and the section-level source comments in the Markdown give the pointers.

**Everything else cited is [PB] or [2°].** No RFC, no standards draft, no paper, and no external repository was fetched while drafting. Where a mechanism is attributed to RFC 6962, to DSSE, to Sagas or to Terraform, the attribution reflects what the internal specification records about its own derivation, not a fresh reading. Before publication, each should be opened and either promoted to [PV] or moved into this appendix.

### C.2 Claims deliberately not made because the source was not reachable

- **The second incident** (a 2026-04 production database and backup deletion via an over-scoped token) is in our internal file with two vendor-blog citations at domain-root granularity, which are not permalinks and cannot be checked. It is not cited in §1 and should not be cited until a resolvable source exists.
- **Venue, volume and page numbers for the Sagas paper, and for the tamper-evident-logging antecedents.** The bibliographic records are believed correct; none was opened for this draft.
- **Star counts and licences in §6.9** were verified through the GitHub API on 2026-08-13 by a separate lane; this drafting lane did not re-run them. Star counts in particular will be stale on the day you read this.
- **The identifier of the adjacent research work on portable action envelopes.** Our internal competitor file records an arXiv identifier for it. The same file records that a different, widely repeated citation in that space turned out to be fabricated, which is precisely the reason an unverified identifier from the same file is not reproduced here.
- **The claim that no prior work sits at our combination.** §6.10 states the qualified version. The unqualified version is not available to anyone and is not asserted.

### C.3 Sections of this draft whose confidence is lowest

Ranked, most doubtful first.

1. **§6, related work.** The weakest section and the one most likely to contain an error of attribution. Every citation is second-hand for this draft (C.1), the agent-side survey is a snapshot with fast-moving numbers, and the framing of what differs between our escrow ordering and standard compensating transactions (§6.4) is our own reading of a literature we did not open. Assume this section is wrong somewhere and treat corrections as the highest-value response to this report.
2. **§1.1's incident narrative.** True in outline on the strength of an aggregator record. The detail about fabricated records in place of deleted ones comes from that aggregator's summary of reports we did not read.
3. **§5.3's fsync attribution.** The claim that the ext4 overrun is fsync cost rather than proxy cost rests on a call count from `strace` times a separately measured unit cost, whose product agrees with the measurement to within about 4%. That is consistent with the explanation; it is not the same as having isolated the cause.
4. **§3.5's claim that window contiguity plus a ledger size bound detects all three shapes of omission.** There are tests for the three named shapes. There is no argument here that the three are exhaustive, and we do not offer one.
5. **§2.4's description of rule two.** Two different rules have carried the number 2 in different internal documents (clock and entropy single-read; single call site for `apply`). Both are real and both are enforced; the numbering is a collision we have reported rather than resolved, and a reader tracing a reference to "rule two" in our source comments may land on either.
6. **§6.9's characterisation of what each surveyed project does not have.** These are statements about the absence of a mechanism in a repository we read at README and design-document level, under a strict prohibition on copying code. Absence at that reading depth is weaker evidence than presence.
7. **§6.8's attribution of the semantic lineage.** Every author and year in that subsection is carried from the internal specification's own reading, not from ours. The one substantive claim in it that is *ours* — that the commutator formulation is a type error outside abelian change structures, and that independence-with-residual is the correct generalisation — is a downgrade the project applied to its own founding intuition after an external mathematical review, and we have reproduced the conclusion rather than re-derived it.
8. **The claim in §2.4 that the two membrane rules make a class of bypass into a near-compile-time failure.** They make it a *test* failure, on a stage that is in the manually run script. Whether that stage runs on a pull request depends on §5.5, and for eleven of thirteen crates it does not.

### C.4 Whole areas this report does not address at all

Named so that their absence is not read as their non-existence:

- **Multi-writer and multi-node operation.** Everything here is single-node. Distributed behaviour of the ledger, of the escrow store, and of the compare-and-swap window is undesigned.
- **Performance under concurrency through the real HTTP surface.** Registered work; the in-process figure is not a substitute.
- **Key management at organisational scale.** Revocation exists and can only be signed by the key being revoked, which has two consequences that are properties rather than bugs: a compromised secret can at worst disavow itself, and a *lost* key can never be revoked. Operator-signed revocation requires a trust root that this design does not have.
- **Privacy of the locator.** A locator string can itself be confidential — a path, a branch name, an internal tool name — and is currently stored and exported in the clear. Selective redaction is a commercial-tier design item.
- **Evidence collector trust.** The gate checks that evidence is signed. It does not check that it is true, and there is a single trust tier: a compromised collector is a compromised input and signing does not help.
- **Cost, pricing, deployment topology, and everything else on the business side.** Out of scope by construction; those documents are not part of the public repository.

### C.5 The standing invitation

The most useful reply to this report, in descending order of value: a counter-example to §6.10; an attribution error in §6; a demonstration that §3.5's three omission shapes are not exhaustive; a case where the declared coverage in §4 is narrower than the actual coverage, or wider.

---

*End of draft v2. Publication is gated on the author, on the trademark and registrar check for the name, and on promoting the citations in §6 from [PB] to [PV].*
