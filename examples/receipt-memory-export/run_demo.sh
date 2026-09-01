#!/usr/bin/env bash
# Example: drive `gx` through a handful of real fs-substrate transformations,
# capture the CLI's own JSON for each, and run export_memory.py over the
# capture to produce agent-memory nodes -- then verify node_count == receipt
# count and round-trip every node's receipt reference back through `gx`.
#
# Copy this into a working checkout (it needs a built `gx` on PATH, or
# GX=/path/to/gx set beforehand) and run it with `bash run_demo.sh`. Like
# demo_one_screen.sh beside it, this is a copy-paste starting point and not a
# job wired into this repository's own CI.
#
# Requires: python3 (docs/TUTORIAL.md already leans on it for JSON field
# extraction; this script does the same rather than adding a jq dependency).
#
# 🔴 Keep $HOME on a real POSIX filesystem while this runs. docs/TUTORIAL.md
# §2 measured `gx key gen` refusing to *load* a key whose store is on a
# filesystem that cannot hold Unix permissions (a Windows drive seen through
# WSL's /mnt/c, a network share) -- the key writes, gx warns the mode is wider
# than 0600, and the next verb that needs it refuses. This script does not
# override $HOME; run it from a shell whose $HOME already satisfies that.
set -eu
GX="${GX:-gx}"
RUN=${1:-1}
root="$HOME/gx_receipt_memory_demo/run$RUN"
rm -rf "$root"
mkdir -p "$root/proj" "$root/captures"
proj="$root/proj"
captures="$root/captures"
export_out="$root/memory"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== receipt-memory-export demo -- run $RUN ==="
echo "project: $proj"

key_json=$($GX key gen --json 2>/dev/null)
key_id=$(printf '%s' "$key_json" | python3 -c 'import json,sys;print(json.load(sys.stdin)["key_id"])')
echo "key: $key_id"

commit_one () {
  # $1 = target file (created if absent), $2 = new contents, $3 = context
  local target="$1" contents="$2" context="$3"
  printf '%s' "$contents" > "$root/intent_$$.txt"
  local o iid tid s
  o=$($GX --project "$proj" submit --substrate fs --locator "$target" \
      --intent "$root/intent_$$.txt" --context "$context" \
      --actor-key "$key_id" --actor-kind agent \
      --actor-model "receipt-memory-export/demo" 2>"$root/err_submit")
  iid=$(printf '%s' "$o" | python3 -c 'import json,sys;print(json.load(sys.stdin)["intent_id"])')
  o=$($GX --project "$proj" plan "$iid" 2>"$root/err_plan")
  tid=$(printf '%s' "$o" | python3 -c 'import json,sys;print(json.load(sys.stdin)["transformation"]["id"])')
  s="${tid//:/_}"
  printf '%s' "$o" > "$captures/$s.plan.json"
  $GX --project "$proj" verify "$tid" >/dev/null 2>"$root/err_verify"
  o=$($GX --project "$proj" commit "$tid" 2>"$root/err_commit")
  printf '%s' "$o" > "$captures/$s.finalize.json"
  rm -f "$root/intent_$$.txt"
  echo "$tid"
}

undo_one () {
  # $1 = transformation id to undo
  local tid="$1" o utid s
  o=$($GX --project "$proj" undo "$tid" 2>"$root/err_undo")
  utid=$(printf '%s' "$o" | python3 -c 'import json,sys;print(json.load(sys.stdin)["transformation"])')
  s="${utid//:/_}"
  printf '%s' "$o" > "$captures/$s.finalize.json"
  # deliberately no $captures/$s.plan.json: a plain `gx undo` has no separate
  # plan step and prints no actor/context/delta (docs/TUTORIAL.md §"Undo it").
  # export_memory.py's input contract makes plan.json optional for exactly
  # this reason and says so explicitly on the resulting node instead of
  # inventing an actor for it.
  echo "$utid"
}

# `gx submit --substrate fs` snapshots the locator at `plan` time (docs/TUTORIAL.md
# §"Plan"), so the file has to exist before the first change to it -- exactly the
# "before any agent touched it" setup line the tutorial itself does. It does not
# have to exist for a *second* write, and t3 below deliberately reuses notes_a.txt
# after t1 to demonstrate that.
printf 'before any agent touched it (a)\n' > "$proj/notes_a.txt"
printf 'before any agent touched it (b)\n' > "$proj/notes_b.txt"

t1=$(commit_one "$proj/notes_a.txt" "first change through gx" "Evidence")
t2=$(commit_one "$proj/notes_b.txt" "second change through gx" "Substrate")
t3=$(commit_one "$proj/notes_a.txt" "third change, same file as t1" "Evidence")
# Undo the MOST RECENT write to notes_a.txt (t3), not t1: DR-43-1's CAS refuses
# an undo whose attested postcondition the world no longer matches, and t3
# already moved the world past what t1 attested. This is a real refusal this
# script's first draft hit (undoing t1 out of order timed out at --settle 120
# and then answered PRECONDITION_CHANGED) -- undo has to walk the same file's
# history LIFO, same as any other append-only log.
u1=$(undo_one "$t3")

echo "committed: $t1 $t2 $t3"
echo "undone:    $u1 (undoes $t3)"

for tid in "$t1" "$t2" "$t3" "$u1"; do
  s="${tid//:/_}"
  $GX --project "$proj" receipt show "$tid" --level 3 --json > "$captures/$s.show.json" 2>"$root/err_show_$s"
done

python3 - "$captures" "$t1" "$t2" "$t3" "$u1" <<'PY'
import json, sys
captures, *tids = sys.argv[1:]
with open(f"{captures}/manifest.json", "w") as f:
    json.dump({"transformations": tids}, f, indent=2)
    f.write("\n")
PY

echo "--- export ---"
python3 "$here/export_memory.py" --in "$captures" --out "$export_out" --openviking-style

echo "--- verify: node_count == committed-transformation receipt count =="
# .gx/receipts also holds *.verdict.json files (one per `gx verify` call -- a
# VerdictReceipt, which never became a memory node: it attests a plan was
# admissible, not that anything committed). Measured on a real run (2026-09-01):
# 3 commits + 1 undo here produced 4 *.commit.json + 3 *.verdict.json = 7 files
# total. Comparing node_count against *all* receipt files would fail for a
# reason that has nothing to do with the exporter being wrong, so this counts
# only the *.commit.json files -- the receipts export_memory.py's own contract
# says it turns into nodes -- and discloses the raw total alongside it instead
# of silently narrowing what "receipt count" means.
receipt_files_total=$(ls "$proj/.gx/receipts"/*.json 2>/dev/null | wc -l | tr -d ' ')
receipt_files_commit_shaped=$(ls "$proj/.gx/receipts"/*.commit.json 2>/dev/null | wc -l | tr -d ' ')
node_count=$(python3 -c "import json;print(json.load(open('$export_out/index.json'))['node_count'])")
manifest_count=$(python3 -c "import json;print(len(json.load(open('$captures/manifest.json'))['transformations']))")
echo "receipt_files_total_on_disk=$receipt_files_total (includes *.verdict.json, not exported) receipt_files_commit_shaped=$receipt_files_commit_shaped node_count=$node_count manifest_count=$manifest_count"
fail=0
[ "$receipt_files_commit_shaped" = "$node_count" ] || { echo "MISMATCH: receipt_files_commit_shaped != node_count"; fail=1; }
[ "$manifest_count" = "$node_count" ] || { echo "MISMATCH: manifest_count != node_count"; fail=1; }

echo "--- verify: round trip each node's receipt reference through gx ---"
python3 -c "
import json
idx = json.load(open('$export_out/index.json'))
for row in idx['nodes']:
    print(row['transformation'])
" | while read -r tid; do
  o=$($GX --project "$proj" receipt show "$tid" --level 1 --json 2>/dev/null)
  seen=$(printf '%s' "$o" | python3 -c 'import json,sys;print(json.load(sys.stdin)["transformation"])')
  if [ "$seen" != "$tid" ]; then
    echo "ROUNDTRIP FAIL: node named $tid, gx receipt show answered $seen"
    fail=1
  else
    echo "roundtrip OK: $tid"
  fi
done

echo "--- verify: supersede evidence is correct for the one undone transformation ---"
python3 -c "
import json
idx = json.load(open('$export_out/index.json'))
nodes = {r['node_id']: json.load(open('$export_out/nodes/' + r['node_id'] + '/node.json')) for r in idx['nodes']}
t3_node = nodes['${t3//:/_}']
u1_node = nodes['${u1//:/_}']
ok = True
se = t3_node['undo']['supersede_evidence']
if se['found_superseding_undo'] is not True or se['superseding_transformation'] != '$u1':
    print('FAIL: t3 supersede_evidence wrong:', se); ok = False
# 🔴 Was: asserted a fixed sentinel ('not_applicable_is_itself_an_undo') that
# export_memory.py never emits -- written before this script had ever been run
# against a real gx build, so the mismatch went undetected (SS855's '急ぐと
# *間違う*', not just late). Measured (2026-09-01, real gx 0.1.0, this run):
# u1's own payload.reversibility carries a real C-25 answer -- 'this undo's own
# invertibility (redo)', per the node's own reversibility_note -- not a fourth
# value. The correct check is that it is one of gx's three real answers, not a
# guess at which one; the actual value is printed either way (self-kill: 全て懐疑,
# 特に成功していると言っている所).
u1_rev = u1_node['undo']['reversibility_at_commit']
print('INFO: u1 (an undo) reversibility_at_commit =', u1_rev, '(answers: can this undo itself be undone / redo)')
if u1_rev not in ('true', 'false', 'unknown'):
    print('FAIL: u1 reversibility_at_commit is not one of the three valid C-25 values:', u1_rev); ok = False
for other in ('${t1//:/_}', '${t2//:/_}'):
    se2 = nodes[other]['undo']['supersede_evidence']
    if se2['found_superseding_undo'] is not False:
        print('FAIL:', other, 'should show found_superseding_undo=False, got', se2); ok = False
print('SUPERSEDE_EVIDENCE_OK' if ok else 'SUPERSEDE_EVIDENCE_FAIL')
" || fail=1

echo "FAILURES=$fail"
echo "RESULT=$([ "$fail" -eq 0 ] && echo PASS || echo FAIL)"
exit $fail
