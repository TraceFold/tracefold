#!/usr/bin/env node
// Copies the one file `tsc` does not: the compiled `.wasm` binary `wasm-gen/wasm_verify.js`
// `require`s at `__dirname`-relative runtime (`fs.readFileSync`). `tsc`'s `allowJs` already copies
// the generated `.js`/`.d.ts` glue into `dist/wasm-gen/` because they are part of the TS program's
// rootDir tree; the `.wasm` itself is neither JS nor TS, so it needs this one explicit step
// (req/132 §5 item 1's build pipeline, package.json's `"build"` script).
//
// 🔴 **E-SDK-9** (`req/503` §1): what this script does **not** do is build that `.wasm`. It copies
// whatever is on the disk, and for four releases what was on the disk was older Rust than the tree
// held. Freshness is not defended here -- a copy step cannot tell a current binary from a stale one
// without compiling the crate, and this script deliberately runs on machines that cannot. It is
// defended by `test/wasm_vocab_freshness.test.mjs`, which reads the vocabulary out of the Rust and
// requires it to be present in the artefact, and which `"pretest"` and `"prepublishOnly"` both
// reach. `npm run build:wasm` (WSL: cargo + wasm-bindgen) is the step that makes it true again.
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = dirname(here);
const src = join(root, "src", "wasm-gen", "wasm_verify_bg.wasm");
const destDir = join(root, "dist", "wasm-gen");
const dest = join(destDir, "wasm_verify_bg.wasm");

if (!existsSync(src)) {
  console.error(
    `copy-wasm: ${src} is missing. Run npm run build:wasm first (sdk/wasm-verify/build.sh -- cargo + wasm-bindgen-cli, WSL only).`,
  );
  process.exit(1);
}
mkdirSync(destDir, { recursive: true });
copyFileSync(src, dest);
console.log(`copy-wasm: ${src} -> ${dest}`);
