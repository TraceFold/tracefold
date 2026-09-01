#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Glovrex
"""Derive agent-memory nodes from `gx` receipts, using CLI output only.

What this is
------------
A demonstration that memory is *downstream* of a signed receipt: every node this
script writes is a pure function of bytes `gx` already printed to stdout for some
CLI verb. It never reads `.gx/` state, never opens the journal or the ledger, and
never imports any `gx-*` crate. If the project this ran against disappeared, the
`captures/` directory alone is enough to reproduce every node byte for byte --
which is the whole claim: memory is *derived*, so it is regenerable, not a second
place of record.

Why there is no `gx --list-receipts` this script calls
--------------------------------------------------------
There isn't one. `gx replay` reconstructs Sigma and reports a match/diff summary,
not a per-record list (crates/gx-cli/src/replay.rs); `gx draft list` covers
undrafted intents, not committed ones; `gx verdict-checkpoint list` lists signed
*counts*, not receipts. Measured by reading `crates/gx-cli/src/main.rs`'s
`Command` enum in full (2026-09-01) -- no verb enumerates a project's committed
receipts. So "list" here is the harness's own manifest of the transformation ids
it drove through `submit -> plan -> verify -> commit|undo` (see `run_demo.sh`),
each backed by real CLI output this script reads back in. This gap is disclosed,
not papered over, per this repo's own doctrine on open disclosure decaying first.

Input contract (per transformation, files gx itself printed)
----------------------------------------------------------------
  captures/<tid>.finalize.json -- REQUIRED. stdout of `gx commit <TID>` or
                                   `gx undo <TID>`, whichever produced this
                                   transformation's receipt.
  captures/<tid>.show.json     -- REQUIRED. stdout of
                                   `gx receipt show <TID> --level 3 --json`.
  captures/<tid>.plan.json     -- OPTIONAL. stdout of `gx plan <ID>` (output is
                                   JSON either way, 44 SS1.3). Present for a
                                   commit (its plan step exists); a plain
                                   `gx undo` has no separate plan step in this
                                   CLI and prints no actor/context/delta at
                                   all (docs/TUTORIAL.md SS"Undo it" -- the
                                   undo output has only envelope/issued_at/
                                   transformation/undone/state/
                                   superseded_state/idempotency_key/
                                   stored_at). When this file is absent the
                                   node says so explicitly rather than
                                   guessing an actor/context/substrate for it.
  captures/manifest.json       -- {"transformations": ["gx1:...", ...]} in the
                                   order the harness drove them (its own
                                   bookkeeping -- not a gx verb, see above)

`<tid>` in a filename is the transformation id with `:` replaced by `_`
(the same substitution `gx` itself uses for `stored_at`, e.g.
`gx1_y7g4jm....commit.json` -- kept for a reason: not a new convention).

Output
------
  memory/nodes/<tid>/node.json     -- the generic node (this script's real output)
  memory/nodes/<tid>/node.md       -- the same content, human-readable
  memory/index.json                -- one row per node, machine-readable
  memory/index.md                  -- the same, human-readable
  memory/openviking_style/...      -- OPTIONAL, only with --openviking-style.
      An addressable-path layout *inspired by* observing OpenViking's L0/L1/L2
      staged-disclosure README (req/998 C-10) and its `viking://` URI style
      (req/998 C-11) -- no OpenViking source was read to build this (COPY HARD
      BAN: observation of documented behaviour is fine, transcribing code is
      not; no OpenViking code exists anywhere in this checkout). It is an
      approximation of a *shape*, not a claim of wire compatibility.

Three-valued fields (this repo's own doctrine, applied to output honestly)
----------------------------------------------------------------------------
`undo.reversibility_at_commit` carries `gx`'s own C-25 answer verbatim
(`true` / `false` / `unknown`) -- see gx-core/src/reversibility.rs. `true` means
an inverse was *constructed* at commit time; it is explicitly NOT a promise that
a later `gx undo` will still succeed (an already-undone transformation is a
known refusal case, per examples/demo_one_screen.sh's own header).

`undo.supersede_evidence` is evidence, not proof: `found` / `not_found` over
*this export's own batch only* (`checked_scope` says the denominator). A
`not_found` here does not mean "still undoable" -- it means this batch's
receipts contain no undo naming this transformation. Absence of evidence
within a bounded batch is written as absence of evidence, never promoted to a
stronger claim (this repo's own "detect gate silence" doctrine, applied to a
python script instead of a CI job).
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def sanitize(tid: str) -> str:
    return tid.replace(":", "_")


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def actor_view(actor_raw) -> dict:
    """Normalize gx's externally-tagged actor enum ({"Agent": {...}}) without
    hardcoding the variant names -- Human/Agent/Process are read from whichever
    key is actually present, so a fourth variant added upstream does not need
    an edit here."""
    if not isinstance(actor_raw, dict) or not actor_raw:
        return {"kind": "unknown", "detail": {}, "raw": actor_raw}
    (kind, detail), = actor_raw.items()
    return {"kind": kind, "detail": detail}


def build_node(tid: str, plan: dict | None, finalize: dict, show: dict) -> dict:
    # `plan` is None (not merely {}) when no captures/<tid>.plan.json existed --
    # distinct from an empty-but-present file, so a bug that writes an empty
    # plan capture is not silently read the same as "no plan step happened".
    plan_available = plan is not None
    txn_raw = (plan or {}).get("transformation")
    txn = txn_raw if isinstance(txn_raw, dict) else {}
    delta = txn.get("delta") or {}
    pfp = (plan or {}).get("precondition_fingerprint") or {}
    payload = show.get("payload") or {}

    receipt_kind = show.get("receipt_kind") or payload.get("receipt_kind")
    reversibility = payload.get("reversibility")
    # 🔴 Measured, not assumed (2026-09-01, against a locally built `gx 0.1.0`):
    # `payload.undo` (the `UndoAttestation` gx-witness/src/receipt.rs documents)
    # was absent/null on every undo receipt this exporter actually produced, and
    # `receipt_kind` read "CommitReceipt" for an undo too -- gx models an undo as
    # a commit-shaped transformation (docs/TUTORIAL.md's own words: "The undo is
    # itself a committed transformation with its own id and its own signed
    # receipt"). What IS reliably present, straight off `gx undo`'s own stdout
    # (captured as this transformation's finalize.json), is the top-level
    # `undone` field naming what it took back. This exporter keys off that
    # instead of the receipt-internal field, so it does not depend on a wire
    # shape this particular build may not populate.
    undoes = finalize.get("undone")
    is_undo = undoes is not None

    node = {
        "schema": "glovrex.receipt-memory-node/v1",
        "node_id": sanitize(tid),
        "receipt_reference": {
            "transformation": tid,
            "canonical_cid": show.get("canonical_cid"),
            "stored_at": finalize.get("stored_at"),
        },
        "when": {
            "issued_at_unix_nanos": show.get("issued_at_unix_nanos"),
            "created_at_unix_nanos": (
                (txn.get("created_at") or txn.get("created_at_unix_nanos"))
                if plan_available
                else None
            ),
            "unit_note": (
                "unix epoch nanoseconds, gx's own wire unit (44 SS0). This exporter "
                "does not convert to RFC 3339 -- a home-made civil-date conversion "
                "would be a date library written badly (the same reason gx-cli's "
                "own `receipt show` leaves the field in nanoseconds)."
            ),
        },
        "what": {
            "receipt_kind": receipt_kind,
            "verdict": show.get("verdict"),
            "context": txn.get("context") if plan_available else "not_available_no_plan_capture",
            "substrate": delta.get("substrate") if plan_available else "not_available_no_plan_capture",
            "locator": pfp.get("scope") if plan_available else "not_available_no_plan_capture",
            "enforced": show.get("enforced"),
        },
        "actor": (
            actor_view(txn.get("actor"))
            if plan_available
            else {
                "kind": "unknown",
                "detail": {},
                "note": (
                    "no captures/<tid>.plan.json for this transformation -- a plain "
                    "`gx undo` prints no actor field at all (docs/TUTORIAL.md SS"
                    "'Undo it'). The signing key_id on the receipt (see "
                    "receipt_reference / gx receipt show --level 2+) is cryptographic "
                    "evidence of who signed, which is a narrower claim than actor "
                    "kind/model."
                ),
            }
        ),
        "undo": {
            "is_undo_of_another_transformation": is_undo,
            "undoes": undoes,
            "reversibility_at_commit": (
                str(reversibility).lower() if reversibility is not None else "unknown"
            ),
            "reversibility_note": (
                "gx's own C-25 attestation (payload.reversibility, gx receipt "
                "show --level 3) for THIS transformation's own invertibility. "
                "On an ordinary commit this is 'can it be undone'; on an undo "
                "(is_undo_of_another_transformation=true) this is 'can the undo "
                "itself be undone', i.e. redo (see examples/demo_one_screen.sh "
                "stage 7). 'true' means an inverse was CONSTRUCTED -- it does "
                "NOT promise a later `gx undo` will still succeed (an "
                "already-undone transformation is a known refusal, per "
                "demo_one_screen.sh's own header)."
            ),
            "supersede_evidence": {
                "checked_scope": "this export's own receipt batch only",
                "found_superseding_undo": None,  # filled in fill_supersede_evidence (needs the full batch)
                "superseding_transformation": None,
            },
        },
        "source": {
            "produced_by": "examples/receipt-memory-export/export_memory.py",
            "consumed_cli_surfaces": [
                "gx plan <ID> --json (captured by the harness at record time)",
                "gx commit|undo <TID> (captured by the harness; whichever produced this receipt)",
                "gx receipt show <TID> --level 3 --json",
            ],
            "engine_internals_touched": False,
        },
    }
    return node


def fill_supersede_evidence(nodes: dict[str, dict]) -> None:
    """Second pass over the whole batch: does any node's receipt name another
    node's transformation as `undoes`? This is why it cannot be done inside
    build_node -- it needs the full batch, not one receipt."""
    undoes_index: dict[str, str] = {}
    for tid, node in nodes.items():
        undoes = node["undo"].get("undoes")
        if undoes:
            undoes_index[undoes] = tid
    for tid, node in nodes.items():
        hit = undoes_index.get(tid)
        node["undo"]["supersede_evidence"]["found_superseding_undo"] = hit is not None
        node["undo"]["supersede_evidence"]["superseding_transformation"] = hit


def node_markdown(node: dict) -> str:
    ref = node["receipt_reference"]
    what = node["what"]
    when = node["when"]
    actor = node["actor"]
    undo = node["undo"]
    lines = [
        f"# memory node {node['node_id']}",
        "",
        f"- receipt: `{ref['transformation']}` (canonical_cid `{ref.get('canonical_cid')}`, stored at `{ref.get('stored_at')}`)",
        f"- what: {what.get('receipt_kind')} / verdict={what.get('verdict')} / context={what.get('context')} / substrate={what.get('substrate')} / locator=`{what.get('locator')}`",
        f"- when: issued_at_unix_nanos={when.get('issued_at_unix_nanos')} (created_at_unix_nanos={when.get('created_at_unix_nanos')})",
        f"- actor: {actor.get('kind')} {actor.get('detail')}",
        f"- undo: reversibility_at_commit={undo.get('reversibility_at_commit')}",
        f"  - supersede evidence ({undo['supersede_evidence']['checked_scope']}): found={undo['supersede_evidence']['found_superseding_undo']} -> {undo['supersede_evidence']['superseding_transformation']}",
        "",
    ]
    return "\n".join(lines)


def build_openviking_style(out_dir: Path, nodes: dict[str, dict]) -> None:
    """Optional, `--openviking-style` only. A directory shape inspired by
    observing (not copying) two documented OpenViking behaviours (req/998
    C-10/C-11): staged L0/L1/L2 disclosure, and an addressable URI-like path.
    No OpenViking source is present in this checkout or was consulted to write
    this function -- README-level observation only, and this says so."""
    root = out_dir / "openviking_style"
    for tid, node in nodes.items():
        base = root / "gx" / "receipts" / node["node_id"]
        base.mkdir(parents=True, exist_ok=True)
        what = node["what"]
        # L0: one-line summary.
        (base / "L0_summary.txt").write_text(
            f"{what.get('receipt_kind')} verdict={what.get('verdict')} "
            f"substrate={what.get('substrate')} locator={what.get('locator')}\n",
            encoding="utf-8",
        )
        # L1: structured overview.
        (base / "L1_overview.json").write_text(
            json.dumps(
                {
                    "what": what,
                    "when": node["when"],
                    "actor": node["actor"],
                    "undo_summary": node["undo"]["reversibility_at_commit"],
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        # L2: full node (already the whole thing -- no compression applied,
        # this is a demo of the *shape*, not a summarization engine).
        (base / "L2_full.json").write_text(
            json.dumps(node, indent=2) + "\n", encoding="utf-8"
        )
        (base / "ADDRESS.txt").write_text(
            f"viking-style-address (observed-shape-only, not a real viking:// scheme): "
            f"gx://receipts/{node['node_id']}\n",
            encoding="utf-8",
        )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--in", dest="in_dir", required=True, type=Path)
    ap.add_argument("--out", dest="out_dir", required=True, type=Path)
    ap.add_argument(
        "--openviking-style",
        action="store_true",
        help="also emit memory/openviking_style/ (observed-shape-only, see module docstring)",
    )
    args = ap.parse_args()

    manifest_path = args.in_dir / "manifest.json"
    if not manifest_path.is_file():
        print(f"ERROR: no manifest at {manifest_path}", file=sys.stderr)
        return 2
    manifest = load_json(manifest_path)
    tids = manifest.get("transformations", [])
    if not tids:
        print("ERROR: manifest lists zero transformations", file=sys.stderr)
        return 2

    nodes: dict[str, dict] = {}
    missing = []
    for tid in tids:
        s = sanitize(tid)
        plan_p = args.in_dir / f"{s}.plan.json"
        fin_p = args.in_dir / f"{s}.finalize.json"
        show_p = args.in_dir / f"{s}.show.json"
        if not (fin_p.is_file() and show_p.is_file()):
            missing.append(tid)
            continue
        plan_json = load_json(plan_p) if plan_p.is_file() else None
        node = build_node(tid, plan_json, load_json(fin_p), load_json(show_p))
        nodes[tid] = node

    if missing:
        print(
            f"ERROR: {len(missing)}/{len(tids)} manifest transformation(s) are "
            f"missing a required finalize.json or show.json capture: {missing}",
            file=sys.stderr,
        )
        return 2

    fill_supersede_evidence(nodes)

    nodes_dir = args.out_dir / "nodes"
    nodes_dir.mkdir(parents=True, exist_ok=True)
    index_rows = []
    for tid, node in nodes.items():
        node_dir = nodes_dir / node["node_id"]
        node_dir.mkdir(parents=True, exist_ok=True)
        (node_dir / "node.json").write_text(
            json.dumps(node, indent=2) + "\n", encoding="utf-8"
        )
        (node_dir / "node.md").write_text(node_markdown(node), encoding="utf-8")
        index_rows.append(
            {
                "node_id": node["node_id"],
                "transformation": tid,
                "receipt_kind": node["what"]["receipt_kind"],
                "verdict": node["what"]["verdict"],
                "substrate": node["what"]["substrate"],
                "issued_at_unix_nanos": node["when"]["issued_at_unix_nanos"],
                "reversibility_at_commit": node["undo"]["reversibility_at_commit"],
                "path": f"nodes/{node['node_id']}/node.json",
            }
        )

    index_rows.sort(key=lambda r: (r["issued_at_unix_nanos"] or 0))
    (args.out_dir / "index.json").write_text(
        json.dumps(
            {
                "schema": "glovrex.receipt-memory-index/v1",
                "node_count": len(index_rows),
                "source_manifest_transformation_count": len(tids),
                "nodes": index_rows,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    md = ["# receipt-memory index", "", f"{len(index_rows)} node(s) derived from {len(tids)} manifest transformation(s).", ""]
    for r in index_rows:
        md.append(
            f"- `{r['node_id']}` -- {r['receipt_kind']} {r['verdict']} on {r['substrate']} "
            f"(reversibility_at_commit={r['reversibility_at_commit']}) -> [{r['path']}]({r['path']})"
        )
    (args.out_dir / "index.md").write_text("\n".join(md) + "\n", encoding="utf-8")

    if args.openviking_style:
        build_openviking_style(args.out_dir, nodes)

    print(
        f"OK: {len(index_rows)} node(s) written for {len(tids)} manifest "
        f"transformation(s) (node_count == source_manifest_transformation_count: "
        f"{len(index_rows) == len(tids)})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
