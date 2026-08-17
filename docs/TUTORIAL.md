# Tutorial (10 minutes)

`gx demo` (see the README) runs a version of this walk for you, offline, in a few seconds.
This page drives the same four stages by hand — install, wrap a real MCP server, read the
receipt, undo, verify offline — so you see the actual commands rather than a black box. It
ends where `gx demo` points you next: [`/limits`](LIMITS.md).

Every command below is real. `tools/verify_p5.sh` runs this same sequence and checks each
step's exit code, so this page cannot go stale without a battery turning red.

## 0. Install

```sh
cargo install --path crates/gx-cli
```

## 1. A sandbox, and a key

```sh
mkdir -p tutorial/sandbox && cd tutorial
echo "before any agent touched it" > sandbox/notes.md
gx key gen --json > pub.json
KEY_ID=$(python3 -c 'import json;print(json.load(open("pub.json"))["key_id"])')
```

`gx key gen` writes the secret half to `~/.gx/keys/$KEY_ID.key` (req/56 §3) — never to
`pub.json`, which is why `pub.json` is the file every offline check below reads.

## 2. Create the project

`gx wrap` opens a project's `.gx/` and does not create it (a long-lived proxy over a
project that already exists). `gx submit` is the verb that creates it — here, once, with a
throwaway draft against the same server the wrap step will actually use:

```sh
printf '{"tool":"notes.write","arguments":{}}' > intent.json
gx --project ./project submit --substrate mcp \
  --locator "stdio://tutorial#file://$PWD/sandbox/notes.md" \
  --intent intent.json --context Substrate --actor-key "$KEY_ID" \
  --actor-kind agent --actor-model "you/1 (typed by hand)"
```

## 3. Wrap a real MCP server

The server here is `gx __demo-notes-server` — the same one-file, two-tool (`notes.write` /
`notes.restore`) MCP server `gx demo` runs automatically, bundled into this one binary so a
tutorial needs no server of your own to install first. It speaks real JSON-RPC 2.0 over
real stdio; nothing about the protocol is simulated.

Three JSON-RPC frames — a handshake, the "I'm ready" notification, and one `tools/call` —
piped in as if an agent were typing them:

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"tutorial","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"notes.write","arguments":{"uri":"file://'"$PWD"'/sandbox/notes.md","contents":"after an agent wrote through gx wrap"}}}' \
  | gx --project ./project wrap --endpoint stdio://tutorial --actor-key "$KEY_ID" \
      --actor-model "you/1 (typed by hand)" \
      --restore notes.write=notes.restore --restore notes.restore=notes.write \
      -- gx __demo-notes-server \
  > wrap.out
```

`wrap.out` holds two JSON-RPC responses: the handshake, and the `tools/call` answer. The
second carries `_meta["gx/transformation"]` — the id everything below is about — and
`_meta["gx/commit"]["stored_at"]` — the receipt's path on disk.

```sh
TID=$(python3 -c '
import json
for line in open("wrap.out"):
    if not line.strip():
        continue
    obj = json.loads(line)
    if obj.get("id") == 2:
        print(obj["result"]["_meta"]["gx/transformation"])
')
echo "$TID"
cat sandbox/notes.md   # "after an agent wrote through gx wrap"
```

## 4. Read the receipt, and publish a checkpoint while it is still the newest entry

```sh
RECEIPT=$(python3 -c '
import json
for line in open("wrap.out"):
    if not line.strip():
        continue
    obj = json.loads(line)
    if obj.get("id") == 2:
        print(obj["result"]["_meta"]["gx/commit"]["stored_at"])
')
gx --project ./project receipt show "$TID" --level 2
gx --project ./project log checkpoint --key ~/.gx/keys/"$KEY_ID".key --out head.json
```

`--level 2` is a summary (verdict, whether it was enforced, whether an inclusion proof
exists). `--level 4` (or `--json`) is the full DSSE envelope — the actual signed bytes,
not a description of them. The checkpoint matters here for the same reason it does in
`gx demo` — an inclusion proof is relative to a **tree size** (RFC 6962), so this receipt
has to be checked against the head that was current when it was issued, not a later one
(step 6 uses this exact file).

## 5. Undo it

An undo is itself a transformation the gate judges (43 §5), which is why it needs the same
`--mcp-restore` declaration the wrap step made and the same `--mcp-endpoint` the original
locator carries:

```sh
gx --project ./project --mcp-server gx --mcp-server-arg __demo-notes-server \
  --mcp-endpoint stdio://tutorial \
  --mcp-restore notes.write=notes.restore --mcp-restore notes.restore=notes.write \
  undo "$TID"

cat sandbox/notes.md   # back to "before any agent touched it"
```

## 6. Verify offline — a separate process, no `.gx/`, no network

This is the step an auditor who was not there for steps 1–5 actually runs: given only a
receipt file and a public key, does it check out?

```sh
gx receipt verify "$RECEIPT" --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

`"valid":true` with `"inclusion":"verified"` means: the signature is real, the canonical
id matches the payload, and this receipt is included in the ledger at the tree size the
checkpoint names — checked from the bytes alone, with no server asked and no network
touched.

If the log has moved on since the receipt was issued, the checkpoint is a head of a
**larger** tree than the one the receipt's proof names, and the two need one more document
between them (RFC 6962 §2.1.2):

```sh
gx --project ./project log consistency --from <receipt tree_size> --to <checkpoint tree_size> > bridge.json

gx receipt verify "$RECEIPT" --offline   --checkpoint head.json --checkpoint-key pub.json --key pub.json   --consistency bridge.json
```

Without it the answer is `"inclusion":"unbridged"` (exit 7) — which is **not** a pass and
**not** an accusation: it says the anchor and the receipt are about different trees and
nothing tied them together. Drop `--offline` and let `gx` read the project's own ledger and
that proof is made for you, so `gx receipt verify "$RECEIPT" --key pub.json` verifies any
receipt in the history, not only the newest.

## 7. Take one copy out of the box, after every commit

Everything up to here happens inside `./project/.gx/`. Every check `gx` performs on itself is
also inside `./project/.gx/`, and that is the whole of what the seventh adversarial audit
measured (`req/232`, 43 §7.9): checks that live in the directory they protect answer accidents
— a crash, a power cut, a file edited by hand, an older tool — and cannot answer somebody who
can write to that directory. They can be deleted, replaced, or rolled back to an older genuine
copy, and the last of those defeats signature checking outright, because a signature says
*who* and never *when*.

What survives is a copy that left the machine:

```sh
gx --project ./project checkpoint export "$HOME/gx-heads/$(date +%Y%m%dT%H%M%S).json"
```

No key is needed — the document is already signed — and the bytes are the same bytes
`gx log checkpoint --out` writes, so `gx receipt verify --offline --checkpoint <FILE>` reads
them with nothing new. The export refuses if the head it would copy does not verify, so a file
that appears in that directory is one `gx` was willing to vouch for.

**How often: after every commit, or at least once a day, whichever is less work to automate.**
A checkpoint taken *before* a range of commits proves nothing about them — the audit measured
exactly that, an export taken too early answering "no rollback here" about a project that had
lost three commits since. Put the directory somewhere the project cannot reach: another
machine, an object store, a colleague's laptop, a printout of the root hash. Keep your commit
receipts (`./project/.gx/receipts/`) beside it; they are signed and unforgeable, and they are
also **deletable**, which is why the copy is what matters.

Two commands say whether history was removed:

```sh
gx --project ./project repair --against "$HOME/gx-heads/<the one you kept>.json"
gx receipt verify "$RECEIPT" --offline --checkpoint "$HOME/gx-heads/<the one you kept>.json" --checkpoint-key pub.json --key pub.json
```

The first refuses (exit 1, `against.rolled_back: true`) when the project's tree is shorter than
the checkpoint you kept, or is a different history of the same length. The second takes a
receipt for a commit that is no longer in the project: `verified` against the checkpoint you
kept and `refuted` (exit 7) against the project's own ledger is a removed commit, stated by two
documents that disagree rather than by an accusation.

If the checkpoint belongs to a *different* project, `gx repair` says so (`against.foreign:
true`) rather than calling your project rolled back — comparing one log's head with another's
tree is a question with no answer, and answering it anyway is how a healthy project gets a
frightening report.

## 8. What this build does not cover

```sh
gx limits
```

Read it before you decide what to trust `gx` with — [`/limits`](LIMITS.md) is the same
eight lines for a browser instead of a terminal.
