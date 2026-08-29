<div align="center">

<a href="https://github.com/TraceFold/tracefold#try-it-yourself"><img src="https://github.com/TraceFold/tracefold/releases/download/brand-assets/banner.png" alt="Tracefold, the inverse is sealed before the action" width="880"></a>

### It asks before the changes it can't put back.

**Tracefold** holds a checked inverse for an agent's change **before** it lands, in Rust. When the
inverse is in hand the change goes through and you are never asked; when one cannot be built, the
agent stops and the question comes to you. Every verdict also becomes a receipt that verifies
offline, without trusting whoever issued it.

<p>
<sub><b>Run it</b></sub><br>
<a href="https://tracefold.github.io/tracefold/verify.html"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/check%20a%20receipt%20in%20your%20browser-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNCAxMi40bDUuMiA1LjJMMjAgNi44Ii8%2BPC9nPjwvc3ZnPg%3D%3D"><img alt="check a receipt in your browser" src="https://img.shields.io/badge/check%20a%20receipt%20in%20your%20browser-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNCAxMi40bDUuMiA1LjJMMjAgNi44Ii8%2BPC9nPjwvc3ZnPg%3D%3D"></picture></a>
<a href="https://github.com/TraceFold/tracefold#flip-one-byte-and-the-verifier-says-no"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/the%2010--second%20demo-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNSA4bDQgNC00IDQiLz48cGF0aCBkPSJNMTIuNSAxNkgxOSIvPjwvZz48L3N2Zz4%3D"><img alt="the 10-second demo" src="https://img.shields.io/badge/the%2010--second%20demo-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNSA4bDQgNC00IDQiLz48cGF0aCBkPSJNMTIuNSAxNkgxOSIvPjwvZz48L3N2Zz4%3D"></picture></a>
<a href="https://github.com/TraceFold/tracefold/tree/main/gui"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/the%20window-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNMyA1aDE4djE0SDN6Ii8%2BPHBhdGggZD0iTTMgOS41aDE4Ii8%2BPC9nPjwvc3ZnPg%3D%3D"><img alt="the window: a GUI over the engine" src="https://img.shields.io/badge/the%20window-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNMyA1aDE4djE0SDN6Ii8%2BPHBhdGggZD0iTTMgOS41aDE4Ii8%2BPC9nPjwvc3ZnPg%3D%3D"></picture></a>
<a href="https://www.npmjs.com/package/@mahirhir/tracefold"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/the%20SDK%20on%20npm-ece7da?style=for-the-badge&labelColor=ece7da&logo=npm&logoColor=0b0a09"><img alt="@mahirhir/tracefold on npm" src="https://img.shields.io/badge/the%20SDK%20on%20npm-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=npm&logoColor=ece7da"></picture></a>
</p>
<p>
<sub><b>Read the limits first</b></sub><br>
<a href="#what-you-cannot-take-from-this"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/the%20limits%20first-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNMyAxMmgxMSIvPjxwYXRoIGQ9Ik0xOCA1djE0Ii8%2BPC9nPjwvc3ZnPg%3D%3D"><img alt="the limits first" src="https://img.shields.io/badge/the%20limits%20first-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNMyAxMmgxMSIvPjxwYXRoIGQ9Ik0xOCA1djE0Ii8%2BPC9nPjwvc3ZnPg%3D%3D"></picture></a>
<a href="#where-it-stands"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/where%20it%20stands%2C%20with%20conditions-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNSAxOXYtNyIvPjxwYXRoIGQ9Ik0xMiAxOVY1Ii8%2BPHBhdGggZD0iTTE5IDE5di00Ii8%2BPC9nPjwvc3ZnPg%3D%3D"><img alt="where it stands, with conditions" src="https://img.shields.io/badge/where%20it%20stands%2C%20with%20conditions-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNSAxOXYtNyIvPjxwYXRoIGQ9Ik0xMiAxOVY1Ii8%2BPHBhdGggZD0iTTE5IDE5di00Ii8%2BPC9nPjwvc3ZnPg%3D%3D"></picture></a>
<a href="https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/technical%20report-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNSA3aDE0Ii8%2BPHBhdGggZD0iTTUgMTJoMTQiLz48cGF0aCBkPSJNNSAxN2g4Ii8%2BPC9nPjwvc3ZnPg%3D%3D"><img alt="technical report" src="https://img.shields.io/badge/technical%20report-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNSA3aDE0Ii8%2BPHBhdGggZD0iTTUgMTJoMTQiLz48cGF0aCBkPSJNNSAxN2g4Ii8%2BPC9nPjwvc3ZnPg%3D%3D"></picture></a>
</p>
<p>
<sub><b>What it is made of</b></sub><br>
<a href="https://github.com/TraceFold/tracefold/blob/main/rust-toolchain.toml"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/rust%201.97.1-ece7da?style=for-the-badge&labelColor=ece7da&logo=rust&logoColor=0b0a09"><img alt="rust, pinned to 1.97.1" src="https://img.shields.io/badge/rust%201.97.1-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=rust&logoColor=ece7da"></picture></a>
<a href="https://github.com/TraceFold/tracefold/tree/main/lean"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/machine--checked%20in%20Lean-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNyA0djE2Ii8%2BPHBhdGggZD0iTTcgMTJoMTAiLz48L2c%2BPC9zdmc%2B"><img alt="machine-checked in Lean" src="https://img.shields.io/badge/machine--checked%20in%20Lean-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNyA0djE2Ii8%2BPHBhdGggZD0iTTcgMTJoMTAiLz48L2c%2BPC9zdmc%2B"></picture></a>
<a href="https://github.com/TraceFold/tracefold/blob/main/LICENSE"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/Apache--2.0-ece7da?style=for-the-badge&labelColor=ece7da&logo=apache&logoColor=0b0a09"><img alt="licence: Apache-2.0" src="https://img.shields.io/badge/Apache--2.0-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=apache&logoColor=ece7da"></picture></a>
<a href="https://github.com/TraceFold/tracefold/commits/main"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/github/last-commit/TraceFold/tracefold?style=for-the-badge&labelColor=ece7da&color=ece7da&logo=github&logoColor=0b0a09&label=last%20commit"><img alt="last commit" src="https://img.shields.io/github/last-commit/TraceFold/tracefold?style=for-the-badge&labelColor=0b0a09&color=0b0a09&logo=github&logoColor=ece7da&label=last%20commit"></picture></a>
</p>
<p>
<sub><b>Ask</b></sub><br>
<a href="https://discord.gg/rtvXqYEQzr"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/discord-ece7da?style=for-the-badge&labelColor=ece7da&logo=discord&logoColor=0b0a09"><img alt="discord" src="https://img.shields.io/badge/discord-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=discord&logoColor=ece7da"></picture></a>
<a href="https://glovrex.com"><picture><source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/badge/glovrex-ece7da?style=for-the-badge&labelColor=ece7da&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjMGIwYTA5IiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNCAxMmgxNiIvPjxwYXRoIGQ9Ik0xMiA3bDUgNS01IDUiLz48L2c%2BPC9zdmc%2B"><img alt="built by Glovrex" src="https://img.shields.io/badge/glovrex-0b0a09?style=for-the-badge&labelColor=0b0a09&logo=data:image/svg%2Bxml;base64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPGcgZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZWNlN2RhIiBzdHJva2Utd2lkdGg9IjEuNyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIj48cGF0aCBkPSJNNCAxMmgxNiIvPjxwYXRoIGQ9Ik0xMiA3bDUgNS01IDUiLz48L2c%2BPC9zdmc%2B"></picture></a>
</p>

</div>

[Try it](#try-it-yourself) ·
[Limits](#what-you-cannot-take-from-this) ·
[The whole chain](#the-whole-chain) ·
[Where it stands](#where-it-stands) ·
[What it does](#what-it-does) ·
[Report](https://github.com/TraceFold/tracefold/blob/main/docs/TRACEFOLD_TR.md) ·
[Contributing](https://github.com/TraceFold/tracefold/blob/main/.github/CONTRIBUTING.md) ·
[Discord](https://discord.gg/rtvXqYEQzr) ·
[Glovrex](https://glovrex.com)

---

## Flip one byte, and the verifier says no

![Verify a receipt, flip one byte, verify again](https://github.com/TraceFold/tracefold/releases/download/demo-assets/tracefold-demo-10s.gif)

Three files are on that terminal: a receipt, a signed checkpoint, a public key. No project
directory, no account, no network call. The receipt verifies and exits `0`. One byte of its
payload is flipped in place, `cmp -l` prints the single line proving exactly one byte moved,
and the same command exits `7`.

Real terminal, captured with `script(1)` against a fresh anonymous clone at commit `177141e3`
on 26 August 2026, WSL2 Ubuntu 24.04. Nothing retyped or staged. Outside the recording on
purpose: `cargo build --workspace`, 64 seconds on that run. Also published as
[asciicast v2](https://github.com/TraceFold/tracefold/releases/download/demo-assets/tracefold-demo-10s.cast).

## Try it yourself

The CLI is not released. There is no `gx` binary to install from a registry, so running the tool
starts from a clone. Checking a receipt somebody hands you is a separate question with a shorter
answer, and that one installs nothing.

**In a browser, with nothing installed:** <https://tracefold.github.io/tracefold/verify.html>

Paste or drop in a receipt and the key it should verify against, plus a checkpoint if you were
given one. The check runs in your tab, on the same WebAssembly build of the same function
`gx receipt verify --offline` calls, so what comes back is the engine's answer rather than a
second opinion about it. If you have no receipt yet, four buttons load real files out of the test
suite. Nothing you paste leaves the page, and you are not asked to take that on trust: the page
wraps `fetch`, `XMLHttpRequest`, `WebSocket` and `sendBeacon` and prints the running count of
requests its own code has made, on screen, where it stays at zero. Save the file, disconnect and
reopen it if you would rather check it that way.

**It answers in three values, not two.** *Verified*, *Refuted*, and a third for a check that did
not conclude, meaning the signature held but no ledger claim was checked, or this build cannot read
that receipt. The third is the one worth having. A two-valued verifier must file "I could not read
this" as either a pass or an accusation, and the command line still collapses it: exit `7` covers
both a tampered receipt and one too new to parse, and the field that separates them sits in the
JSON body an exit code cannot carry. The page reads that body, so it does not call a document
forged when the truth is that it failed to read it.

What the page cannot do is make a receipt. It checks one. Producing one is still the clone below.

**What the clone costs you today, stated plainly.** You need a Rust toolchain (stable, pinned in
`rust-toolchain.toml`) and roughly a minute of compile time. On Windows the documented path is
WSL, and a WSL install is not free: the virtual disk grows with the build tree, and on this
project's own machine it reached hundreds of gigabytes. If that is more than you want to spend
to check one receipt, that is a reasonable place to stop, and it is the honest state today.

**If you write JavaScript, there is a second shipped route.** `@mahirhir/tracefold` is on npm under
Apache-2.0 and carries that same WebAssembly verifier, so checking a receipt costs a package
install and six lines, with no Rust toolchain and no WSL. It also talks to a running `gx serve`,
which the browser page does not.

```sh
npm i @mahirhir/tracefold
F=https://raw.githubusercontent.com/TraceFold/tracefold/main/crates/gx-cli/tests/fixtures/attach_face_frozen/issued_2026_08_22
curl -sO $F/commit_receipt.json -O $F/key.pub.json -O $F/checkpoint.json
```

```js
import { readFileSync } from "node:fs";
import { verifyReceiptOffline } from "@mahirhir/tracefold";

const key = JSON.parse(readFileSync("key.pub.json", "utf8"));
const result = verifyReceiptOffline(
  readFileSync("commit_receipt.json", "utf8"),
  key.key_id,
  key.public_key,
  readFileSync("checkpoint.json", "utf8"),
  key.key_id,
  key.public_key,
);

console.log(result.valid, result.checks.inclusion, result.anchor_authenticated);
// true verified true
```

It never throws. A bad signature, a wrong key, a malformed document and an argument that is not a
string all arrive as `{valid: false, error: "..."}`, because whether a receipt is good is an
answer, not an exception. Drop the last three arguments and you get `valid: false` with
`inclusion: "unanchored"`, the CLI's own refusal to call a receipt verified when it cannot place
it in a log.

The module it loads declares exactly one host import, and that import is a table initialiser, so
there is no socket and no clock inside it. It is 423,910 bytes, sha256 `8f7064b675cc89e5...`, the
same bytes `sdk/wasm-verify/` builds in this tree. Offline is a property of the artifact you can
check rather than a promise on this page.

One route is still unfinished: prebuilt binaries would remove the compile step for the CLI itself.
That has not landed, so treat it as in progress rather than as a plan you can use, and this page
will not pretend otherwise.

**Signing, when binaries exist.** macOS builds are intended to be codesigned and notarized, so
a Mac user gets a double-clickable binary and no unidentified-developer dialog; the developer
account for it is in hand. Windows is not the same story. An EV certificate is a real recurring
cost that is not being paid yet, so a Windows binary would raise a SmartScreen warning, and the
honest options there are to document that warning and how to proceed past it, or to build from
source. No Windows signing is being promised.

```sh
git clone https://github.com/TraceFold/tracefold
cd tracefold
cargo build --workspace          # about a minute, and the slowest step here
export PATH="$PWD/target/debug:$PATH"
```

Make one change the way an agent would, through the gate, so there is a receipt to check:

```sh
mkdir -p walk && cd walk
echo "before any agent touched it" > notes.md
gx key gen --json > pub.json
KEY_ID=$(python3 -c 'import json;print(json.load(open("pub.json"))["key_id"])')

printf 'after an agent wrote through gx\n' > intent.txt
gx --project . submit --substrate fs --locator "$PWD/notes.md" \
  --intent intent.txt --context Substrate --actor-key "$KEY_ID" \
  --actor-kind agent --actor-model "readme/1" > submit.json
IID=$(python3 -c 'import json;print(json.load(open("submit.json"))["intent_id"])')

gx --project . plan "$IID" > plan.json
TID=$(python3 -c 'import json;print(json.load(open("plan.json"))["transformation"]["id"])')
gx --project . verify "$TID" > verify.json
gx --project . commit "$TID" > commit.json

RECEIPT=$(python3 -c 'import json;print(json.load(open("commit.json"))["stored_at"])')
gx --project . log checkpoint --key ~/.gx/keys/"$KEY_ID".key --out head.json
```

Now check it the way a stranger would, with no workspace and no network:

```sh
gx receipt verify "$RECEIPT" --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

Exit `0`. Then flip one byte of the receipt and run the identical command against the copy:

```sh
python3 - "$RECEIPT" <<'EOF'
import re, sys
raw = open(sys.argv[1], "rb").read()
m = re.search(rb'"payload": ?"([A-Za-z0-9+/=]+)"', raw)
i = m.start(1) + 40
b = raw[i:i+1]
open("tampered.json", "wb").write(raw[:i] + (b"A" if b != b"A" else b"B") + raw[i+1:])
EOF
cmp -l "$RECEIPT" tampered.json          # exactly one line
gx receipt verify tampered.json --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

Exit `7`. Every command on this page was run in that order on a fresh clone before being
printed here.

**If it exits `0` instead, that is a finding, and we would rather have it than not.** The same
goes for anywhere the tool turns out to claim more than it actually checked. You are now
holding the three files a verification needs and the binary that reads them, which is
everything required to attack the claim: open an issue with what you ran, and it gets recorded
here whatever it costs us.

**Pass `--checkpoint-key`.** Without it the check still exits `0`, but the answer says
`"anchor_authenticated":false`: the checkpoint was read and not authenticated, so you
verified against a file you took on faith. Drop `--checkpoint` entirely and you get `7` with
`"inclusion":"unanchored"`, the tool refusing to call a receipt verified when it cannot place
it in a log.

Exit codes are the contract a script branches on without parsing JSON: `0` valid, `7` does
not verify or is unanchored, `6` not found, `1` bad input, `2` refused by policy. Full table
in `crates/gx-cli/src/exit.rs`. The subcommands that exist today: `submit`, `plan`, `verify`,
`commit`, `wrap`, `attach`, `demo`, `limits`, `confine`, `verdict-checkpoint`, `receipt`,
`log`, `checkpoint`, `key`, `undo`, `cancel`, `escalation`, `policy`, `repair`, `replay`,
`draft`, `serve` — the list is `enum Command` in `crates/gx-cli/src/main.rs`, minus one
hidden internal helper. `gx limits` prints the gaps below at a terminal.

**Free forever for one person.** Receipt generation, offline verification and self-hosting
are unlimited and unexpiring for a single person using this alone, which is a promise rather
than a price.

## What you cannot take from this

Five classes of failure sit outside this **by declaration rather than oversight**. They are
above the features because reading them first can save you the afternoon.

| out of scope | why it cannot be closed from the inside |
|:--|:--|
| Root or kernel-privileged writes | They bypass the tool entirely, and this build does not detect that |
| Writes into the tool's own state directory | A detector living in that directory cannot judge it. The defence is an artifact held elsewhere |
| A policy encoding the wrong intent | It is enforced faithfully. No verification reaches the question of whether the rule was right |
| Undoing one change and not another, across objects | Today the unit is a single transformation, and the check is a compare-and-set on the same object. If one change was made after reading another, nothing here records that it was read, so there is no way to ask for one back without the other |
| An issuer who cuts the tail off the chain | A hash chain proves that what you hold has not been edited. It cannot prove that what you hold is all there was. An issuer who hands you a genuine but older checkpoint, with the last entries removed, produces something that verifies. Detecting that needs a newer checkpoint from somewhere the issuer does not control, which is what an external anchor is for, and we do not publish to one yet |

The demonstration above has a limit worth saying in the same breath as the claim. Exit `7`
proves the receipt you hold is not the receipt that was signed. It says nothing about whether
the change the receipt describes was the change anyone wanted.

**One more thing worth knowing before you start rather than after.** What you can later
select on is fixed at the moment of capture, not at the moment you ask. A field that was not
recorded when the change landed cannot be recovered as a filter afterwards, so the set of
questions you can put to the history only ever grows forward from the day you begin.

The full list ships in [`docs/LIMITS.md`](docs/LIMITS.md), and a test fails if it drifts from
the code that enforces it. These are not sentences someone remembered to update.

## The whole chain

The atom above is one command against one file. This is why it matters: the receipt travels,
and it still holds in someone else's hands.

![An agent acts, a receipt is issued, three files travel, a stranger verifies offline, a tampered copy fails](https://github.com/TraceFold/tracefold/releases/download/demo-assets/tracefold-demo-30s.gif)

An agent changes a file through `gx`. The change is described, planned, judged and committed,
and leaves a signed receipt. Three files travel to somebody who was not there and does not
trust the sender: receipt, checkpoint, public key. They verify offline and get `0`, tamper
with one byte, and get `7`. Nothing else moved, and no service was asked to vouch for
anything. Same recording conditions as above.

> [!IMPORTANT]
> **Not released, with one exception named here.** The bare names `tracefold` on npm and on
> crates.io are ours and are taken, but what sits behind both is an empty 0.0.1 placeholder
> holding the name, published on 12 and 13 August 2026. Installing either gets you nothing.
> The exception is the scoped `@mahirhir/tracefold` 0.1.0, published 25 August 2026, which is
> a real package and the one the JavaScript above installs: it verifies receipts and does not
> contain the engine. There is no released `gx` binary anywhere, the tool itself is still a
> build from source, and nothing above should be read otherwise. The download counters on the
> two placeholder pages are mirrors and scanners fetching a new name once: 110 of the 125 npm
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

## Where it stands

| | measured | under what conditions |
|:--|--:|:--|
| Test floor | **2,639** | probes across 464 suites, plus the SDK's 36 passed / 0 failed / 7 skipped. The SDK line and the fresh-clone conditions are 25 August 2026's frozen-harness run; the probe and suite counts are 26 August 2026's, reconstructed from the tree by `floor_doubt` f1 rather than re-measured on a fresh clone, and the two halves are dated separately because they were taken separately. This floor has moved more than forty times in a month; it moves with every repair round |
| Machine-checked | **117** | theorems in Lean, 12 of them counterexamples, out of 118 line-initial declarations. The remaining one is an `axiom`, a statement assumed rather than proved, and it is named in the report. No `sorry`, the keyword standing in for a proof nobody wrote, so there are none. Proof rather than bounded model checking: nothing here is true only up to a scope · re-counted on a fresh clone 26 August 2026 |
| Open holes | **0** | high severity, open as of 25 August 2026, counted as accepted findings whose repair has not been accepted. This number was 3, then 0, then 1, then 0 again inside a week, so read a zero here as the state of one afternoon rather than a property of the system. Forty-four adversarial rounds have landed, plus independent B-band and S(1) audits, all at zero as recorded |
| Not measured | **3** | Windows native, OneDrive, SMB. Zero runs out of the three, as of 25 August 2026 |

The commands that produce the first, second and fourth, all runnable in a clone:

```sh
# test floor, and the SDK line separately
bash tools/e2e.sh
cd sdk/typescript && npm ci && npm test
# theorems, then the one assumed statement; add up the per-file counts
grep -rcE '^theorem' lean/GxSpec.lean lean/GxSpec/*.lean
grep -rcE '^axiom' lean/GxSpec.lean lean/GxSpec/*.lean
# the unmeasured surfaces, declared rather than discovered
grep -n "Windows, OneDrive" docs/LIMITS.md
```

Those greps are anchored to the start of the line on purpose. Allowing leading whitespace
returns 119 theorems and 2 axioms, but the three extra hits are English sentences wrapped
inside doc comments, where a line happens to begin with the word:

```
lean/GxSpec/Attribution.lean:49:  theorem below asserts that gx's running implementation ...
lean/GxSpec/MinimalityF0.lean:41:  theorem here strengthens, weakens or restates any frozen ...
lean/GxSpec/MinimalityF0.lean:32:  axiom set stays `{propext, Quot.sound, GxSpec.composeId}` ...
```

None is a declaration. Counting the loose way would add a theorem the prover never saw and a
second unproved assumption that does not exist, so 117 and 1 are the numbers above.

**The third row has no command, and that is not an oversight.** An open-hole count is read off
the audit ledger, and no single invocation produces it. So that row says where the number
comes from and you are taking it on our word. A count that only ever falls is a count someone
is managing rather than measuring.

**What would show this is wrong.** Two things, and either one is enough. Produce a receipt
that `gx receipt verify` accepts while the inverse it names does not restore the state it
claims to restore. Or land a change through the gate that leaves no receipt. Both are
checkable by someone who does not trust us, which is the point; if you find either, open an
issue and it will be recorded here whatever it costs us.

**Deliberately absent:** no build badge, and the honest reason is worse than not having one.
`ci.yml` is configured to run on every push, but no job has actually started since
2026-08-15: GitHub rejects them before execution with "recent account payments have failed
or your spending limit needs to be increased". The runs last two to five seconds and produce
no logs. So there is no signal here at all -- not a green one, not a red one -- for the 2,245
commits since. `cargo check --workspace --all-targets` passes locally, which is a person's
run, not a machine's. No download counts, no star totals; neither measures whether the thing
works.

## What it does

Four behaviours, and they are not four settings you configure separately. One declaration
names an object and what may happen to it; the gate that runs before the action, the rule
enforced while it runs, and the fields attested after it all read out of that declaration.
That shape is not a discovery of ours and is being arrived at independently elsewhere. The
narrower difference: a receipt here carries an inverse constructed and checked *before* the
change landed, which is not a field reporting afterwards that an action was reversible.

**Escrow before commit.** Where an inverse can be constructed it is constructed, checked, and
stored durably *before* the change is applied. Undo is a checked property, not an assumption
made afterwards.

**Measured, not self-reported.** A fingerprint of the substrate is taken before and after a
change reaches the object a transformation names, so what happened is measured rather than
described by the same process that did it.

**Offline-verifiable receipts.** Every verdict, admit or deny or escalate, is signed and
anchored in an append-only log, and re-checks with no network and no trust in the issuer.

**Declared coverage.** What is not covered ships beside what is. A skip prints its name
rather than passing quietly.

## Figures you can re-derive

Numbers about a project are worth what it costs you to check them, so the commands sit here
rather than the claims. Run them in a clone at any commit and you get whatever is true at
that commit, which may differ from what is printed below.

```sh
# implementation, excluding the test trees
find crates -name '*.rs' -not -path '*/tests/*' -print0 | xargs -0 cat | wc -l
# the test trees
find crates -path '*/tests/*' -name '*.rs' -print0 | xargs -0 cat | wc -l
# direct dependency surface, once the toolchain is installed
cargo tree --depth 1 -e normal
```

On 26 August 2026, at commit `177141e3`, the first printed 80,647 and the second 139,966,
across 142 and 362 files in 13 crates. The tests are larger than the thing they test, and
that is the only claim these figures support. Our own ledger counts test lines a second way,
excluding files merely named for tests, and on 18 August the two rules disagreed by eight per
cent on the same tree on the same day. That is why the command sits above the number and the
date sits beside it.

The dependency surface is real: the third command lists it, and every entry is code you would
be trusting on our recommendation.

## Read further

Two walkthroughs go slower than this page, with every command executed and printed:

- [Flip one byte and the verifier exits 7](docs/articles/tamper-evident-receipts.md), the
  full tamper table and every exit code from `0` to `7`.
- [Verifying an AI agent's actions offline](docs/articles/verify-ai-agent-actions-offline.md),
  what an audit trail here contains and what it is blind to.

[`docs/TUTORIAL.md`](docs/TUTORIAL.md) drives the same steps by hand against a real MCP
server. [`docs/TRACEFOLD_TR.md`](docs/TRACEFOLD_TR.md) is the long form: the calculus, the
receipt format, what was measured and under which conditions, related work graded by how well
it was checked, and every non-claim this project makes about itself.
[`examples/ci/receipt-check.yml`](examples/ci/receipt-check.yml) is a copy-pasteable GitHub
Actions job that fails a pull request when a delivery's receipt is missing or does not verify.

## Contributing

Open an issue before a large change, and bring a measurement. The rules are short and are in
[`CONTRIBUTING.md`](.github/CONTRIBUTING.md); the shortest version is that a pull request
which lowers a count, skips a suite or narrows an assertion has to say so in its own
description. Silently bounded is the failure this project guards against hardest.

Good first things to pick up: connecting an agent framework via an adapter
([#1](https://github.com/TraceFold/tracefold/issues/1)), Python bindings
([#2](https://github.com/TraceFold/tracefold/issues/2)), a limit that is true but badly
worded, a platform in the "not measured" row above, or a self red-team probe that breaks
something we believe holds. Open RFCs and adapter proposals sit in
[Issues](https://github.com/TraceFold/tracefold/issues).

Questions and half-formed ideas belong in an issue, or in [Discord](https://discord.gg/rtvXqYEQzr) if you would rather
think out loud. The invite above is the one we intend to keep; if you find it expired, that is a
bug in this page and an issue about it is welcome.

## Sponsors

None, and none are being solicited yet. If this ends up load-bearing for your work and you
want it to keep being maintained,
[say so](https://github.com/TraceFold/tracefold/issues); knowing who depends on it changes
what gets prioritised more than money would at this stage.

## License

Apache-2.0 © Glovrex. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for attribution of
incorporated work.
