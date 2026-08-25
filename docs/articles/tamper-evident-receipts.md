# Flip one byte and the verifier exits 7

A tamper-evident receipt is a record that fails verification if any part of it changes, even
by one byte. This page demonstrates that on a real receipt: `gx receipt verify --offline`
exits `0` on the intact receipt, and exits `7` after a single byte of the receipt's payload
is flipped. Every command below was executed, in order, on a fresh clone of this repository
(commit `cf143ac3`, 2026-08-26, WSL2 Ubuntu 24.04). If one of them doesn't do what this page
says, that's a bug in the product or the page: open an issue and it gets recorded.

Two limits before anything else, because they bound what the demonstration means. Exit 7
proves the receipt you hold isn't the receipt that was signed; it doesn't prove the change
the receipt describes was the change you wanted, and it can't see a change that bypassed the
tool entirely (root-privileged writes, for one). The full list is in
[`LIMITS.md`](../LIMITS.md), and it's short enough to read first.

## Build it

```sh
git clone https://github.com/TraceFold/tracefold
cd tracefold
cargo build --workspace
export PATH="$PWD/target/debug:$PATH"
```

The build finished in 59 seconds on the run recorded here (dev profile). The binary is `gx`.
Not released; there's nothing to install from a registry, which is why this page starts from
a clone.

## Make a change worth receipting

This is the shortest complete road `gx` has: one plain file, one key, no server. It's the
same walk as [`TUTORIAL.md`](../TUTORIAL.md) §2, compressed. The point of the six verbs is
that the change is described before it's made, judged against policy, and applied only with
a signed receipt and an escrowed inverse.

```sh
mkdir -p fs-walk && cd fs-walk
echo "before any agent touched it" > notes.md
gx key gen --json > pub.json
KEY_ID=$(python3 -c 'import json;print(json.load(open("pub.json"))["key_id"])')

printf 'after an agent wrote through gx\n' > intent.txt
gx --project . submit --substrate fs --locator "$PWD/notes.md" \
  --intent intent.txt --context Substrate --actor-key "$KEY_ID" \
  --actor-kind agent --actor-model "article/1 (typed by hand)" > submit.json
IID=$(python3 -c 'import json;print(json.load(open("submit.json"))["intent_id"])')

gx --project . plan "$IID" > plan.json
TID=$(python3 -c 'import json;print(json.load(open("plan.json"))["transformation"]["id"])')
gx --project . verify "$TID" > verify.json
gx --project . commit "$TID" > commit.json

RECEIPT=$(python3 -c 'import json;print(json.load(open("commit.json"))["stored_at"])')
gx --project . log checkpoint --key ~/.gx/keys/"$KEY_ID".key --out head.json
```

`notes.md` now holds the new text, `$RECEIPT` points at a signed commit receipt under
`.gx/receipts/`, and `head.json` is a signed checkpoint of the ledger head, taken while this
receipt is the newest entry.

## Verify it offline

```sh
gx receipt verify "$RECEIPT" --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

Exit `0`, and this on stdout:

```json
{"valid":true,"checks":{"signature":true,"canonical_cid":true,"inclusion":"verified",
 "revocation":"not_consulted"},"key_id":"ed25519-b86f091ed3c45e1e",
 "anchor":"checkpoint-file","anchor_authenticated":true,"retroaction":null,
 "issued_at_signed":false,"issued_at_unix_nanos":1787682983973000420}
```

Three files went into that check: the receipt, the checkpoint, the public key. No `--project`,
no network, no trust in whoever issued the receipt. That's the whole offline verification
contract, and it's what the rest of this page attacks.

## Flip one byte

The receipt's payload is base64. Flip one character of it, in place, leaving every other byte
of the file alone:

```sh
python3 - "$RECEIPT" <<'EOF'
import re, sys
raw = open(sys.argv[1], "rb").read()
m = re.search(rb'"payload": ?"([A-Za-z0-9+/=]+)"', raw)
i = m.start(1) + 40
b = raw[i:i+1]
flip = b"A" if b != b"A" else b"B"
open("tampered.json", "wb").write(raw[:i] + flip + raw[i+1:])
EOF
cmp -l "$RECEIPT" tampered.json
```

`cmp -l` prints exactly one line, which is the point of doing it this way:

```
 138 116 101
```

One byte differs, at offset 138: octal `116` (`N`) became `101` (`A`). Now verify the
tampered copy with the same command:

```sh
gx receipt verify tampered.json --offline \
  --checkpoint head.json --checkpoint-key pub.json --key pub.json
```

Exit `7`:

```json
{"valid":false,"checks":{"signature":false,"canonical_cid":null,"inclusion":null,
 "revocation":null},"key_id":"ed25519-b86f091ed3c45e1e","anchor":"checkpoint-file",
 "anchor_authenticated":true,"issued_at_signed":false,
 "issued_at_unix_nanos":1787682983973000420,
 "refusal":"no valid signature under key \"ed25519-b86f091ed3c45e1e\" (34 AC-019)"}
```

Note the order of the checks. `signature` is `false` and everything after it is `null`: once
the signature fails there's nothing trustworthy left to check, so nothing else is checked.

## What each exit code means

Exit statuses are specified because things branch on them. An agent harness, a CI job or a
shell script gets the same eight answers a human does:

| exit | meaning |
|-----:|:--|
| 0 | reached the intended state; for `receipt verify`, the receipt holds |
| 1 | error: invalid input, internal error, or adapter error |
| 2 | refused: the gate answered `Deny`, and only that |
| 3 | precondition mismatch: the world changed between plan and commit |
| 4 | escalated: a person has to rule before this proceeds |
| 5 | apply failed |
| 6 | not found |
| 7 | offline verification failure (`gx receipt verify` only) |

`2` is worth a sentence: clap's default exit for a usage error is 2, and this CLI deliberately
remaps usage errors to `1` so that a mistyped flag can never read as "the gate refused your
change". A test asserts that no usage error takes exit 2.

## Every other way this was broken, same session

Each row below was executed against the same walk, on the same clone, the same day.

| tamper | exit |
|:--|--:|
| one byte flipped in the payload base64 | 7 |
| one byte flipped in the signature base64 | 7 |
| one character changed in the checkpoint's `root_hash` | 7 |
| verified against a freshly generated wrong public key | 7 |
| receipt path that doesn't exist | 6 |
| a change targeting `/etc/hostname`, at `gx verify` (policy `fs-deny-etc`) | 2 |
| `gx undo` of the committed change, then `cat notes.md` | 0, and the file is back |

The `/etc` row is the other half of the design: the same machinery that makes a receipt
tamper-evident refuses some changes outright, before they happen, and the refusal is itself
a signed, verifiable record.

## What exit 7 doesn't prove

It doesn't prove the system is safe; it proves this receipt was altered. A receipt that
verifies is still only as meaningful as what was allowed to happen: a policy that encodes the
wrong intent is enforced faithfully, and an issuer who truncates the tail of the ledger hands
you something that still verifies against its own shortened head, which is why anchoring the
checkpoint outside the issuer matters and is stated as an open limit. Those and the rest are
in [`LIMITS.md`](../LIMITS.md), above the features, on purpose.

For where these receipts come from and why the tool refuses when it can't build an inverse,
see [Verifying an AI agent's actions offline](verify-ai-agent-actions-offline.md), the
[README](../../README.md), and the [technical report](../TRACEFOLD_TR.md).
