<div align="center">

<a href="https://github.com/TraceFold/tracefold"><img src="https://github.com/TraceFold/tracefold/releases/download/brand-assets/banner.png" alt="Tracefold" width="900"></a>

# TraceFold

**AI agents should not make irreversible changes.**
**TraceFold escrows the inverse before an effect lands — or refuses the effect — and issues receipts anyone can verify offline.**

Not an observability tool. It sits before the effect.

See the Scope Exclusions table below and [docs/LIMITS.md](docs/LIMITS.md) for what this does not cover.

<p>
<a href="https://www.npmjs.com/package/@mahirhir/tracefold"><img src="https://img.shields.io/badge/npm-SDK%20v0.1-0070f3?style=for-the-badge&logo=npm&logoColor=ffffff" alt="npm SDK"></a>
<a href="https://crates.io/crates/tracefold"><img src="https://img.shields.io/crates/v/tracefold.svg?style=for-the-badge&logo=rust&logoColor=ffffff&label=crates.io&color=7928ca" alt="crates.io"></a>
<a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/Rust-1.97.1-7928ca?style=for-the-badge&logo=rust&logoColor=ffffff" alt="Rust 1.97.1"></a>
<a href="https://doi.org/10.5281/zenodo.22168558"><img src="https://img.shields.io/badge/DOI-10.5281%2Fzenodo.22168558-0070f3?style=for-the-badge&logo=doi&logoColor=ffffff" alt="Zenodo DOI"></a>
<a href="mailto:mahirohirakawa@glovrex.com"><img src="https://img.shields.io/badge/Contact-mahirohirakawa%40glovrex.com-26231f?style=for-the-badge&logo=minutemailer&logoColor=ffffff" alt="Email"></a>
<a href="https://discord.gg/rtvXqYEQzr"><img src="https://img.shields.io/badge/Community-Discord-5865F2?style=for-the-badge&logo=discord&logoColor=ffffff" alt="Discord"></a>
<a href="https://buy.stripe.com/8x214mbkU2pu5yneyk97G00"><img src="https://img.shields.io/badge/Support-Pay%20what%20you%20want-635bff?style=for-the-badge&logo=stripe&logoColor=ffffff" alt="Support Tracefold"></a>
</p>

</div>

---

## A real agent write, escrowed and reversed

```
┌────────────────────────────────────────────────────────────────────────────┐
│ 🔴 🟡 🟢  gx x OpenClaw — real tool call, escrowed and reversed             │
├────────────────────────────────────────────────────────────────────────────┤
```
<div align="center">
<img src="https://github.com/TraceFold/tracefold/releases/download/demo-assets/tracefold-demo-openclaw-undo.gif" alt="A real DeepSeek-backed OpenClaw agent issues a write tool call, gx's before_tool_call escrow hook fires before the write lands, the write commits, gx undo reverses it, and the restored file is byte-identical to the original hash" width="100%" />
</div>

A real OpenClaw agent (`deepseek/deepseek-chat`, a live API call, not a fixture) dispatches a write tool call. gx's `before_tool_call` hook fires and hands the write to escrow before it lands.
- **After the write commits**: `gx undo <txid>` issues a new transformation — nothing is deleted, the write is superseded.
- **Byte check**: `sha256sum` on the restored file matches the pre-write hash exactly.
- **`gx receipt verify --pretty`**: signature, canonical CID, and inclusion all check `true` — offline, no trust required.

[▶ View Asciinema Cast (Raw Timings)](https://github.com/TraceFold/tracefold/releases/download/demo-assets/tracefold-demo-openclaw-undo.cast)

**Reproduction (deterministic, no live LLM call):** [`examples/openclaw-plugin-demo/`](examples/openclaw-plugin-demo/README.md) runs the same `before_tool_call` escrow path — refuse before it lands, take it back after — against a real `gx` binary and a real filesystem, scripted so it does not need an API key.

---

## Quickstart

<table width="100%">
<tr>
<td width="50%" valign="top">
<h4>🌐 Route A: In Your Browser (Zero Install)</h4>
<p>Open <b><a href="https://tracefold.github.io/tracefold/verify.html">tracefold.github.io/tracefold/verify.html</a></b></p>
<p>Paste a receipt and key. Runs 100% via in-tab WebAssembly (network requests strictly 0).</p>
</td>
<td width="50%" valign="top">
<h4>📦 Route B: Node.js / TypeScript (3 Lines)</h4>
<pre><code>npm i @mahirhir/tracefold</code></pre>
</td>
</tr>
<tr>
<td colspan="2" valign="top">
<h4>⌨️ Route C: The <code>gx</code> Command Line (Rust)</h4>
<pre><code>cargo install tracefold</code></pre>
<p>Installs one binary, <code>gx</code>, the CLI the demo above runs (<code>gx undo</code>, <code>gx receipt verify</code>). Published on crates.io at 0.1.2 under Apache-2.0, and it builds from source. No prebuilt binaries yet, tracked in <a href="https://github.com/TraceFold/tracefold/issues/7">#7</a>.</p>
</td>
</tr>
</table>

```js
import { readFileSync } from "node:fs";
import { verifyReceiptOffline } from "@mahirhir/tracefold";

const key = JSON.parse(readFileSync("key.pub.json", "utf8"));
const result = verifyReceiptOffline(
  readFileSync("commit_receipt.json", "utf8"), key.key_id, key.public_key,
  readFileSync("checkpoint.json", "utf8"), key.key_id, key.public_key
);

console.log(result.valid, result.checks.inclusion); // true "verified"
```

---

## The Paradigm Shift: Pre-Fact Provenance

| Dimension | Traditional Post-Hoc Audit Logs | TraceFold Pre-Fact Provenance |
| :--- | :--- | :--- |
| **Execution Order** | Action executes first $\rightarrow$ Logged afterwards | Inverse constructed & checked $\rightarrow$ **Action lands** |
| **Irreversible Damage** | Discovered only after system corruption | **Blocked at the gate**; escalates to human approval |
| **Verification Trust** | Must trust the host/server that produced the log | **Zero-trust offline verification** via standalone WASM |
| **Verdict Precision** | Binary (Pass/Fail) conflates errors with attacks | **Tri-state**: `Verified`, `Refuted`, `Unknown/Unparseable` |

---

## Architecture Flow

```text
[ 01. AI Agent Action ]
         │
         ▼
[ 02. Deterministic Gate ] ──(Cannot build inverse S⁻¹)──► [ 🔴 Halt & Escalate to Human ]
         │
    (Inverse S⁻¹ sealed)
         │
         ▼
[ 03. Action Lands & Receipt Issued ]
         │
         ▼
[ 04. Offline WASM Verifier ] ──► Exit 0 (Verified) / Exit 7 (Refuted)
```

---

## System Context: Glovrex Digital World (Target Architecture)

TraceFold implements the **Deterministic Approval Gate (Layer 3: Mechanical Laws)** and the **Receipt & Escrow Substrate (Layer 8: Provenance & Receipt)** within the broader [Glovrex Digital World](https://github.com/Glovrex) computing architecture (*Paper DOI*: [`10.5281/zenodo.22168558`](https://doi.org/10.5281/zenodo.22168558)).

<div align="center">
<img src="https://raw.githubusercontent.com/TraceFold/tracefold/main/assets/glovrex_target_architecture_vision.png" alt="Glovrex Target Architecture Vision" width="100%">
</div>

---

## Formal Status & Scope Boundaries

| Dimension | Measured Value | Conditions & Scope |
| :--- | --: | :--- |
| **Test Floor** | **2,926** probes passing | 510 suites (2,988 total probes exist in source; 59 failing — self-measured 2-way split this pass: 44 unmet live-DB/prebuilt-binary preconditions in the measuring run (not code regressions), 15 real currently-open gaps; 3 ignored) + SDK 36 passed &middot; fresh clone &middot; 2 Sep 2026 &middot; point-in-time — this number has moved on nearly every commit since; run `tools/e2e.sh` yourself for the count as of your clone, do not treat it as current; freshness of this row is machine-checked by `tools/gates/test_floor_freshness_gate.mjs` |
| **Lean Formal Proofs** | **154** theorems | Lean 4, 14 files: **154 theorems, 14 of them counterexamples, 1 axiom, sorry 0**, recounted 1 Sep 2026 with the attribute-aware predicate (SS1001-consistent; see docs/LIMITS.md item 8) |
| **Open High Holes** | **0** | Out of 44 adversarial audit rounds, same 25 Aug 2026 table/commit as above (not independently re-dated by this pass) |
| **Unmeasured Platforms** | 3 environments | Windows native, OneDrive, SMB |

**Deliberately absent:** no CI/build badge on this page, and the honest reason is worse than not
having one. This repository has never carried a workflow file. No commit has ever touched
`.github/workflows/`, and every Actions run visible here is GitHub's own default setup, CodeQL and
Pages, which has been completing on pushes since 1 Sep 2026. The CI the next sentence describes is
the one in the private tree this repository is synced from, which is also why `f65aac2f` does not
resolve here: it ran zero jobs on any push from 2026-08-15T17:25:29Z while the account was
billing-blocked (`req/908`), and the last time it ran, at that commit, it covered 16 of this
project's 17 workspace crates automatically. The TypeScript SDK's tests have never run under CI at
all. Nothing on this page was produced by a build on this repository, so run `tools/e2e.sh` in your
own clone for a number that came from your machine. Repository state checked against the GitHub API
on 4 Sep 2026; see [docs/LIMITS.md](docs/LIMITS.md) item 8 for the full, dated detail.

### Scope Exclusions (Limits by Design)
| Out of Scope | Why it cannot be closed from inside |
| :--- | :--- |
| **Root or kernel-privileged writes** | Bypasses the tool entirely at the operating system level |
| **Writes into tool's own state dir** | A detector living in that directory cannot judge itself |
| **Policies encoding the wrong intent** | Enforced faithfully; intent correctness is external |

<details>
<summary><b>▶ Expand Environment, Verification & Technical Specifications</b></summary>
<br>

- **Docs Index**: Every document and article in this repo, in reading order, plus a tutorial not linked below: [docs/README.md](docs/README.md).
- **Formal Technical Report**: Complete mathematical proof and receipt encoding: [docs/TRACEFOLD_TR.md](docs/TRACEFOLD_TR.md).
- **Foundational Paper**: [*A Mechanical World Model for Agents* (DOI: 10.5281/zenodo.22168558)](https://doi.org/10.5281/zenodo.22168558).
- **Formal Verification Spec**: Machine-checked Lean 4 theorem suite: [lean/README.md](lean/README.md).
- **Exclusion Taxonomy**: Test-enforced scope boundaries and limits: [docs/LIMITS.md](docs/LIMITS.md).
- **Adapter Guide**: What it takes to connect a new substrate to the escrow gate, measured cost, and what conformance requires: [docs/ADAPTER_GUIDE.md](docs/ADAPTER_GUIDE.md).
- **Error Classification**: Error taxonomy and exit code specifications: [docs/ERROR_TAXONOMY.md](docs/ERROR_TAXONOMY.md).
- **Recoverability Mechanics**: Reversible execution state definitions: [docs/RECOVERABILITY.md](docs/RECOVERABILITY.md).
- **Development Tree Tests**: Full-spectrum test probe taxonomy: [docs/DEVELOPMENT_TREE_TESTS.md](docs/DEVELOPMENT_TREE_TESTS.md).
</details>

---

## db/

[`db/`](db/README.md) is a self-contained crate that compiles a corpus of markdown into addressable
atoms, records what it admitted in an append-only journal, and answers queries from a SQLite index
it can delete and rebuild. Apache-2.0, like the rest of this repository. It is its own cargo
workspace rather than a member of the one at the root, so `cargo build --workspace` here does not
touch it and none of this repository's checks cover it -- build and test it on its own with
`cd db && cargo test -p db` (21 controls, all passing when it was added). On the corpus it was
measured against it returns 14.7x fewer bytes than `grep` for the same question but takes about
62 ms where `grep` takes 2; that trade, and the two rows the harness refused to time at all, are in
[db/README.md](db/README.md).

---

## Writing

- [The undo has to exist before the write does](https://dev.to/mahirhir/the-undo-has-to-exist-before-the-write-does-46on) — 30 Aug 2026
- [What "undo" actually means when the target is a real repo, not a fixture](https://dev.to/mahirhir/what-undo-actually-means-when-the-target-is-a-real-repo-not-a-fixture-4l1c) — 30 Aug 2026
- [I ran 10,373 mutations through a reversibility gate. Tamper detection caught 600 of 600.](https://dev.to/mahirhir/i-ran-10373-mutations-through-a-reversibility-gate-tamper-detection-caught-600-of-600-1bo6) — 31 Aug 2026

---

## Support

Everything shipped here is free — verification, receipts, self-hosting — and stays free for as long as a single person uses it. Nothing is for sale, and paying changes nothing about what you get.

If you want to fund the work anyway, there is exactly one channel:

**[Support TraceFold — pay what you want, via Stripe](https://buy.stripe.com/8x214mbkU2pu5yneyk97G00)**

Changed from Polar to Stripe on 1 Sep 2026 (the Polar checkout account's onboarding was not finishing). Nothing is promised in return either way.

---

<div align="center">
<sub>Direct: <a href="mailto:mahirohirakawa@glovrex.com">mahirohirakawa@glovrex.com</a> &middot; Built by <a href="https://glovrex.com">Glovrex</a> &middot; Licensed under Apache-2.0</sub>
</div>
