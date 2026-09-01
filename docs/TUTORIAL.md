# Tutorial (10 minutes)

This page drives the whole loop by hand — submit, plan, verify, commit, then verify the receipt
offline and flip one byte of it to watch the verifier refuse it — so you see the actual commands
rather than a black box. (Builds carrying the `mcp` feature also have `gx demo`, which runs a
version of this walk for you in a few seconds; the published binary deliberately does not, and
this page is the walk.) It ends at [`/limits`](LIMITS.md).

There are **two** walks here and they are the same loop on two substrates. [§2](#2-the-same-loop-on-one-plain-file--no-server)
is `--substrate fs`: one plain file, no server, and the shortest complete road from `submit` to
a verified `undo`. §3–§7 are `--substrate mcp`: the same road with a real MCP server behind a
proxy. Start with §2 unless you came here to wrap a server — it is the one most first runs
actually want, and reading only the MCP one is what taught the first person to walk this page
two wrong habits (a relative `--locator`, and a JSON intent file on a substrate that does not
parse one).

Every command below is real, and neither walk can go stale without a battery turning red:
`tools/verify_p5.sh` drives §3–§7's sequence and checks each step's exit code, and
`crates/gx-cli/tests/r21_tutorial_fs_walk.rs` extracts §2's own shell blocks **out of this
file** and runs them.

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

## 2. The same loop on one plain file — no server

`--substrate fs` is the shortest complete walk `gx` has: one file, one key, no server and no
JSON-RPC. It drives the same verbs the MCP walk below does — `submit` → `plan` → `verify` →
`commit` → `receipt` → offline verify → `undo` — so read it first if you want to see the loop
before you see the plumbing. Every block below is a real run, and the output printed under each
one is that run's own bytes (only the DSSE `payload` and `sig` base64 is elided at `…`, because
it is long and it is different every time).

Two things about `fs` that this page did not used to say, and that a first run gets wrong. Both
were measured, on this binary, by a reader who had only the README and this file:

* **`--locator` must be an absolute path.** A relative one is accepted by `gx submit` and
  refused two verbs later at `gx plan`, because the locator grammar belongs to the adapter and
  `submit` never reaches one. v0.1 names positions from the root so that a signed record cannot
  depend on which directory a process happened to be in.
* **`--intent` for `fs` is the new contents of the file, as raw bytes.** It is not JSON, and
  nothing parses it. See [the warning](#the-one-mistake-that-costs-you-something) after the walk.

This section stands alone — it makes its own key. If you have already done step 1, you can skip
the `gx key gen` line and reuse that `$KEY_ID` and `pub.json` instead.

### A file, and a key

```sh
mkdir -p fs-walk && cd fs-walk
echo "before any agent touched it" > notes.md
gx key gen --json > pub.json
cat pub.json
KEY_ID=$(python3 -c 'import json;print(json.load(open("pub.json"))["key_id"])')
```

```
{"key_id":"ed25519-1a244850cc9fc5c9","public_key":"GiRIUMyfxcmMgfIhaBxoGL+c4mj1sz+b0iCbqMasusw="}
```

`gx key gen` also prints a line to **stderr** saying where it put the secret half
(`~/.gx/keys/$KEY_ID.key`). That is stderr and not stdout, so `> pub.json` really does get a
single clean JSON object — but `2>&1` would not, and neither does a terminal, which is why it
looks like two lines when you run it by hand.

If `$HOME` is on a filesystem that cannot hold Unix permissions — a Windows drive seen through
WSL's `/mnt/c`, a network share — `gx` writes the key, warns that its mode is wider than `0600`,
and then **refuses to load it** at `gx verify`. Keep the key store on a real POSIX filesystem.

### Submit: describe the change, do not make it

```sh
printf 'after an agent wrote through gx\n' > intent.txt
gx --project . submit --substrate fs --locator "$PWD/notes.md" \
  --intent intent.txt --context Substrate --actor-key "$KEY_ID" \
  --actor-kind agent --actor-model "you/1 (typed by hand)" > submit.json
cat submit.json
IID=$(python3 -c 'import json;print(json.load(open("submit.json"))["intent_id"])')
```

```
{"intent_id":"gx1:32cstxojumridjfujomfg3xcizrpiccewpmyoeqpb4uacrfoqxla","id":null,"order":0,"subject":null,"delta":null,"target":null,"context":"Substrate","actor":{"Agent":{"key":"ed25519-1a244850cc9fc5c9","model":"you/1 (typed by hand)"}},"parents":[],"created_at_unix_nanos":1787083154932085818,"state":"Draft"}
```

`--project .` puts `.gx/` in the directory you are in, and `submit` is the verb that creates it.
Nothing on the substrate has moved — `notes.md` still says `before any agent touched it`, and
will until `commit`. Almost every field is `null` because a `Draft` is an intent and not yet a
change: what it *would* do is not known until something looks at the file.

### Plan: look at the object, and fix the id

```sh
gx --project . plan "$IID" > plan.json
cat plan.json
TID=$(python3 -c 'import json;print(json.load(open("plan.json"))["transformation"]["id"])')
```

```
{"transformation":{"id":"gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","intent_id":"gx1:32cstxojumridjfujomfg3xcizrpiccewpmyoeqpb4uacrfoqxla","order":0,"subject":{"Object":"gx1:2c4pbkicn34zsgwyjvkpms4dqstfvmhgiwriswflwq2lgtvampvq"},"target":null,"delta":{"substrate":"Fs","cid":"gx1:2degqbxsq7n3unt5nra7cskj2ectjubemzqbbjsj56bzvtgsnp7a"},"context":"Substrate","actor":{"Agent":{"key":"ed25519-1a244850cc9fc5c9","model":"you/1 (typed by hand)"}},"parents":[],"created_at":1787083155026251179},"delta_ref":{"substrate":"Fs","cid":"gx1:2degqbxsq7n3unt5nra7cskj2ectjubemzqbbjsj56bzvtgsnp7a"},"precondition_fingerprint":{"substrate":"Fs","scope":"/mnt/c/work/r21_walk/notes.md","digest":"gx1:cgq6hqn5rbtdbsnicnswnf5o6gmwkx5ggyiasfbtphp56uuc32mq"},"state":"Candidate"}
```

This is the step that reads the file. `delta` is now the change that would be made,
`precondition_fingerprint` is what the file looked like when it was read, and
`transformation.id` is fixed from here on — everything below names it.

Give `plan` a **relative** locator and this is where it stops instead:

```
{"type":"https://glovrex.dev/errors/adapter-error","title":"the substrate adapter refused this operation","gx_code":"ADAPTER_ERROR","detail":"the adapter refused to snapshot: the substrate would not answer for \"notes.md\": not a position from the root; v0.1 names positions absolutely (ASM-69-3)"}
```

### Verify: ask the gate

```sh
gx --project . verify "$TID"
```

```
{"kind":"Admit","transformation":"gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","state":"Admitted","proof":{"policy_decisions":[{"policy_id":"fs-permit-default","decision":"Allow","diagnostics_digest":null}],"invariant_results":[],"evidence_digests":[],"composed_from":[],"proof_ref":null},"reasons":null,"enforced":true,"fail_posture_engaged":false,"ticket":null,"held_by":null,"record_only":false,"reverified":true,"receipt_stored_at":"./.gx/receipts/gx1_y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa.verdict.json"}
```

Confirmed by code reading, not a fresh CLI run, 2026-09-01 commit: `Admit` is one of three answers, and the shipped `fs` policy pack is what produced it:
`fs-permit-default` allows, and `policies/fs/deny-etc.cedar` forbids anything under `/etc`. The
other two are exit **2** (`Deny` — try the same walk with `--locator /etc/hostname`) and exit
**4** (`Escalate`, which raises a ticket a person has to rule on with `gx escalation
approve|reject` before `commit` will run). The one way to reach `Escalate` on `fs` in v0.1 is to
overwrite a file whose **current** contents are over 1 MiB: `gx` cannot escrow an inverse that
large, and rather than make a change nobody could take back it stops and asks. `verify` is
read-only either way — it does not touch `notes.md`.

### Commit: apply it, and sign what happened

```sh
gx --project . commit "$TID" > commit.json
cat commit.json
cat notes.md
RECEIPT=$(python3 -c 'import json;print(json.load(open("commit.json"))["stored_at"])')
```

```
{"envelope":{"payload_type":"application/vnd.glovrex.receipt+dagcbor","payload":"q2ZrZXlfaWR4GGVkMjU1MTktMWEyNDQ4NTBjYzlmYzVjOWd2ZXJkaWN0…","signatures":[{"keyid":"ed25519-1a244850cc9fc5c9","sig":"b4N9gey6H8kVUlJ2AayN8EOpuThAmbjAcCo8QdzWzjfvAp4cYrGxsWcGiz0dDBjN0Q0PGB6ikPlBbEonJrU5Cg=="}]},"issued_at":1787083155229412505,"transformation":"gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","state":"Committed","enforced":true,"idempotency_key":"gx-commit:gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","stored_at":"./.gx/receipts/gx1_y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa.commit.json"}
after an agent wrote through gx
```

The file has changed, and `stored_at` is the receipt for it. Before applying anything, `gx`
built and escrowed the **inverse** — which is what makes the undo below possible, and what it
tells you it cannot do when the old contents are too large to escrow.

### Read the receipt, and publish a checkpoint

```sh
gx --project . receipt show "$TID" --level 2
gx --project . log checkpoint --key ~/.gx/keys/"$KEY_ID".key --out head.json
```

```
{"level":2,"transformation":"gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","receipt_kind":"CommitReceipt","verdict":"Admit","enforced":true,"key_id":"ed25519-1a244850cc9fc5c9","canonical_cid":"gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","fail_posture_engaged":false,"has_inclusion_proof":true,"issued_at_unix_nanos":1787083155229412505,"stored_kind":"commit"}
{"origin":"glovrex-ledger/v1","tree_size":1,"root_hash":"gx1:4gkw5tyasuts3blb7eckvy66mym5q7jjxgc6ef336it4mzhpgvkq","timestamp":1787083155438631903,"signature":{"keyid":"ed25519-1a244850cc9fc5c9","sig":"lLU0aZNjd4ou56j6IQMYuGNlFsz5FI9mYj2EtXgT1ENO2pg6sKfXdEC3vsCnquoke3DgnB6Ys1V7wgr7/FLTDg=="}}
```

Take the checkpoint **now**, while this receipt is still the newest entry: an inclusion proof is
relative to a tree size, so `head.json` has to be the head that was current when the receipt was
issued (`tree_size` here is 1, and so is the proof's). §7 says what to do when it is not — the
answer is a consistency proof between the two tree sizes, not a newer checkpoint.

### Verify it offline — no project, no network

```sh
gx receipt verify "$RECEIPT" --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

```
{"valid":true,"checks":{"signature":true,"canonical_cid":true,"inclusion":"verified","revocation":"not_consulted"},"key_id":"ed25519-1a244850cc9fc5c9","anchor":"checkpoint-file","anchor_authenticated":true,"retroaction":null,"issued_at_signed":false,"issued_at_unix_nanos":1787083155229412505}
```

Three files — the receipt, the checkpoint, the public key — and no `--project` anywhere. This is
the command an auditor runs on a machine that has never seen your project.

### Undo it

```sh
gx --project . undo "$TID"
cat notes.md
```

```
{"envelope":{"payload_type":"application/vnd.glovrex.receipt+dagcbor","payload":"q2ZrZXlfaWR4GGVkMjU1MTktMWEyNDQ4NTBjYzlmYzVjOWd2ZXJkaWN0…","signatures":[{"keyid":"ed25519-1a244850cc9fc5c9","sig":"…"}]},"issued_at":1787083155…,"transformation":"gx1:peljgtuzq5ydd4q4yxyd6sc4akssipoyv6dv7vsnrbf2peumapxa","undone":"gx1:y7g4jmyxwobkywvgcdduu44qkvh7utjdft3akrtnjketrwxhdrsa","state":"Committed","superseded_state":"Superseded","idempotency_key":"gx-undo:gx1:peljgtuzq5ydd4q4yxyd6sc4akssipoyv6dv7vsnrbf2peumapxa","stored_at":"./.gx/receipts/gx1_peljgtuzq5ydd4q4yxyd6sc4akssipoyv6dv7vsnrbf2peumapxa.commit.json"}
before any agent touched it
```

`gx undo` also prints a `gx undo settle: …` line to stderr saying whether it waited for the world
to settle before firing. The undo is itself a committed transformation with its own id and its
own signed receipt — `undone` names the one it took back, and that one is now `Superseded`. The
history grows forwards; nothing is rewritten.

🔴 On `fs`, `undo` needs **none** of the `--mcp-server` / `--mcp-restore` / `--mcp-endpoint`
flags §6 below requires. Those are the MCP adapter's: the inverse of a tool call is another tool
call, so it needs a server to send it to. The inverse of a file write is bytes `gx` already has.

### The one mistake that costs you something

The intent file for `fs` is **the new contents of the file**. Nothing reads it as JSON, nothing
validates it, and there is no shape it can be in that `gx` will refuse. So if you copy the
`{"tool":…,"arguments":{…}}` shape from the MCP examples on this page — which is the natural
thing to do, and is what the first person to walk this page actually did — you get this:

```sh
printf '{"tool":"notes.write","arguments":{"contents":"hello"}}' > wrong.json
gx --project . submit --substrate fs --locator "$PWD/notes.md" --intent wrong.json \
  --context Substrate --actor-key "$KEY_ID" --actor-kind agent --actor-model "you/1" > w1.json
IID2=$(python3 -c 'import json;print(json.load(open("w1.json"))["intent_id"])')
gx --project . plan "$IID2" > w2.json
TID2=$(python3 -c 'import json;print(json.load(open("w2.json"))["transformation"]["id"])')
gx --project . verify "$TID2" > /dev/null
gx --project . commit "$TID2" > /dev/null
cat notes.md
```

Confirmed by code reading, not a fresh CLI run, 2026-09-01 commit: every one of those four verbs exits **0**, and the last line prints:

```
{"tool":"notes.write","arguments":{"contents":"hello"}}
```

`notes.md` now literally holds that JSON string — planned, verified, admitted and signed, because
it *is* a perfectly legal file write and `gx` never claimed to know what you meant. This is the
one place on this page where doing the wrong thing is silent, and it is worth saying plainly:
**gx is a guard on whether a change is allowed and a record of what it was — not a check that the
change is the one you wanted.** What it does give you is the way back, and that is one command:

```sh
gx --project . undo "$TID2" > /dev/null
cat notes.md
```

```
before any agent touched it
```

## 3. Create the project

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

## 4. Wrap a real MCP server

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

## 5. Read the receipt, and publish a checkpoint while it is still the newest entry

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
(step 7 uses this exact file).

## 6. Undo it

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

## 7. Verify offline — a separate process, no `.gx/`, no network

This is the step an auditor who was not there for steps 1–6 actually runs: given only a
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

## 8. Take one copy out of the box, after every commit

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

## 9. What this build does not cover

```sh
gx limits
```

Read it before you decide what to trust `gx` with — [`/limits`](LIMITS.md) is the same
eight lines for a browser instead of a terminal.
