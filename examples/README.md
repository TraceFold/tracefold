<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright (c) 2026 Glovrex -->

# examples/

One example lives here today: a copy-paste GitHub Actions workflow that fails CI when a
delivery has no `gx` receipt, or the receipt does not verify offline.

## Contents

- `ci/receipt-check.yml` — verifies a receipt + checkpoint + trusted-key bundle with
  `gx receipt verify --offline`. It is a starting point to copy into your own
  `.github/workflows/`, not a job wired into this repository's own CI (this repo verifies
  its own receipts through its test suite instead). See the file's own header for what it
  assumes and does not cover, and `docs/TUTORIAL.md`'s "Verify offline" section for the
  same check walked by hand.

This folder ships to the public repo unfiltered — `tools/pub_sync_dryrun.sh`'s
`build_manifest()` stages everything `git ls-files -- examples/` returns, with no
whitelist and no `_`-prefix exclusion. Whatever is added here later ships too; nothing
beyond `ci/receipt-check.yml` is implied to exist yet.

---
Derived from: `git ls-files -- examples/` (1 file, 2026-08-30). req/968 P-968-4.
