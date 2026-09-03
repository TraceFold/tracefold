# tracefold-tui

The `gx tui` terminal face: seven `GET` routes rendered to a buffer, and no engine. Four of them are the browser face's own set, kept at four so the two faces stay comparable; two more are read when one record is opened; the seventh is the event stream the live view follows. The four and the seven answer different questions and neither is a typo for the other: four is the browser-parity denominator, seven is everything this face reads.

Part of [Tracefold](https://github.com/TraceFold/tracefold), a defensive agent-safety layer that
makes AI-agent tool execution reversible and independently verifiable (escrow-before-apply, an
append-only Merkle receipt log, offline verification with no issuer round-trip, and an explicit
`docs/LIMITS.md` for what the layer does not cover).

This crate is the terminal renderer only: it consumes `gx-api`'s HTTP surface and nothing else — no
`gx-core`, no engine, no policy evaluation. `cargo tree -e normal -p tracefold-tui` names no other
`tracefold-*`/`gx-*` crate, which is the property this split exists to make structural rather than a
claim about source.

License: Apache-2.0. See the crate's `[lib]` doc comment (`src/lib.rs`) for the extraction rationale.

## Running it

`gx` is not on `PATH`. Every working invocation names the binary by absolute path — the one this
seat has verified is recorded in `/home/testuser2/gx_verified_bin`, and nothing is promoted into
that marker except after a green suite (a build once reached the Owner's screen because a script
sorted by mtime, which knows when a binary was made and nothing about whether it was checked).

**Attach to the face that is already running.** This starts nothing and cannot disturb the engine:

```
wsl -d Ubuntu-24.04 -- tmux attach -t rttui
```

**Start a face of your own** against the running engine. The token is read out of its file rather
than typed, so it never reaches a shell history or a process argument list:

```
wsl -d Ubuntu-24.04 -- bash -lc \
  'GX_BASE_URL=http://127.0.0.1:8858 \
   GX_TOKEN=$(cat /home/testuser2/gx_r9_secrets/token.txt) \
   /home/testuser2/gx_e0064ce0_verified/gx tui'
```

**Choose how the ledger is laid out** by naming a scheme in front of that command. The three are
declarations in `tokens.rs`, not code paths, so a fourth is a table entry rather than a branch:

```
GX_TUI_PLACEMENT=ledger    # every row spelled out (the default)
GX_TUI_PLACEMENT=compact   # the same rows, narrower
GX_TUI_PLACEMENT=digest    # rows that agree are counted instead of repeated
```

**Do not start the engine.** It is already running, and the command that started it is recorded
below as a *fact about the running process*, not as a step to follow. Typing it yourself gets:

    cannot bind 127.0.0.1:8858: Address already in use (os error 98)

which is the binary correctly refusing — and the Owner hit exactly that, because an earlier draft
of this section put the command in a code block among the ones you are meant to run. A command
shown in the same shape as an instruction will be read as one.

The engine (pid 604 at the time of writing) was launched as:

    gx serve --bind 127.0.0.1:8858 --project /home/testuser2/gx_r28_bed
             --token-file /home/testuser2/gx_r9_secrets/token.txt
             --signing-key ed25519-d57dd5c44e974fdb

Restarting it has emptied the bed it serves before now, so it is left alone even when something
looks wrong on the face.

A face reads; it never writes. If `tmux attach` reports no session the face is not running — the
engine may well still be, so ask it before assuming anything died:

```
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8858/v1/healthz
```
