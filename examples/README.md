<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# examples/

Copy-paste starting points. Nothing here is wired into this repository's own build — these are
files you take away and adapt.

---

| File | What it does | What it does not do |
| :--- | :--- | :--- |
| [`ci/receipt-check.yml`](ci/receipt-check.yml) | A GitHub Actions job that fails the build when a delivery carries no `gx` receipt, or carries one that does not verify offline. It runs `gx receipt verify --offline` over a receipt, a checkpoint and a trusted-key bundle. | It is not run by this repository (this repo verifies its own receipts through its test suite instead), and it assumes you supply the key bundle. The file's own header states what it assumes in full. |
| `README.md` | This page. | — |

---

## Using it

Copy `ci/receipt-check.yml` into your own `.github/workflows/`, point it at the receipt and key
your delivery produces, and it will refuse a build whose evidence does not check out.
[`docs/TUTORIAL.md`](../docs/TUTORIAL.md) walks the same verification by hand first, which is the
faster way to find out what the three input files are before automating them.

## Scope of this folder

This folder is published whole: every file added here later is published too, with no allow-list
filtering it. That is why it holds exactly the two files listed above and nothing incidental —
there is no staging step to catch a stray file.
