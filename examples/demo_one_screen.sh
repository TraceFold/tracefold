#!/usr/bin/env bash
# Example: one-screen, 8-stage reproduction of gx's commit / receipt / tamper / undo / redo
# cycle, against a real cloned git repository (not a fixture).
#
# Copy this into a working checkout of this repo (it needs the built `gx` binary) and run it
# with `bash examples/demo_one_screen.sh`. It is a copy-paste starting point, not a wired job
# in this repository's own CI -- this repo verifies its own receipts through its test suite.
#
# This example exercises the git substrate only; the filesystem substrate is not covered here.
#
# What it does, in order:
#   0. record a before/after byte diff on a target file in the clone
#   1. submit that diff and seal an inverse for it
#   2. commit -- the mutation lands
#   3. take a checkpoint and verify the commit's receipt against it
#   4. flip one byte on a COPY of that receipt (the original is left untouched)
#   5. verify the tampered COPY and confirm it is rejected
#   6. undo -- the original state comes back
#   7. redo, by undoing the undo itself -- the mutation comes back
# Each of the three receipts (commit / undo / redo) is then verified again offline, against a
# checkpoint taken at its own time -- a single later checkpoint shared across all three does not
# work, because the log's inclusion proof rejects a checkpoint taken well past an entry's own
# tree size.
#
# Known scope note: undoing the same intent a second time is expected to fail (not found), once
# that intent has already been undone once. This script only ever undoes each intent once, so
# that case does not come up here; it is a known edge of the undo primitive, not something this
# script exercises or repairs.
set -u
GX="${GX:-gx}"   # override: GX=./target/debug/gx bash examples/demo_one_screen.sh
REPO_URL=https://github.com/stevemao/left-pad
REPO_NAME=left-pad
RUN=${1:-1}
root="$HOME/gx_demo_one_screen/$REPO_NAME.run$RUN"
rm -rf "$root"; mkdir -p "$root/home"
export HOME="$root/home"
proj="$root/proj"
now(){ date +%s.%N; }
el(){ awk -v a="$1" -v b="$2" 'BEGIN{printf "%.3f", b-a}'; }
fail=0

t_start=$(now)
echo "=== one-screen 8-stage demo -- run $RUN -- repo=$REPO_NAME substrate=git ==="

git clone --depth 1 -q "$REPO_URL" "$root/clone" || { echo "CLONE_FAILED"; exit 90; }
head=$(git -C "$root/clone" rev-parse HEAD)
br=$(git -C "$root/clone" rev-parse --abbrev-ref HEAD)
target="$root/clone/README.md"
loc="$root/clone#$br:README.md"

# --- stage 0: before/after ---
t0a=$(now)
before_sha=$(sha256sum "$target" | cut -d' ' -f1)
before_bytes=$(wc -c < "$target")
cp "$target" "$root/goal.txt"
printf '\n<!-- gx demo: one change through the membrane, then reverted, then redone -->\n' >> "$root/goal.txt"
goal_bytes=$(wc -c < "$root/goal.txt")
t0b=$(now)
echo "0 before/after [$(el $t0a $t0b)s]: target=README.md before_sha=${before_sha:0:12} before_bytes=$before_bytes goal_diverges_by=$((goal_bytes-before_bytes))bytes"

key_json=$($GX key gen --json 2>/dev/null)
key_id=$(printf '%s' "$key_json" | sed -n 's/.*"key_id":"\([^"]*\)".*/\1/p')
key_file="$HOME/.gx/keys/$key_id.key"
printf '%s' "$key_json" > "$root/pub.json"

# --- stage 1: seal the inverse (submit -> plan -> verify) ---
t1a=$(now)
o=$($GX --project "$proj" submit --substrate git --locator "$loc" --intent "$root/goal.txt" --context Evidence --actor-key "$key_id" 2>"$root/e1")
iid=$(printf '%s' "$o" | sed -n 's/.*"intent_id":"\([^"]*\)".*/\1/p')
o=$($GX --project "$proj" plan "$iid" 2>"$root/e2")
tid=$(printf '%s' "$o" | sed -n 's/.*"id":"\(gx1:[^"]*\)".*/\1/p'|head -1)
o=$($GX --project "$proj" verify "$tid" 2>"$root/e3")
pol=$(printf '%s' "$o" | sed -n 's/.*"policy_id":"\([^"]*\)","decision":"\([^"]*\)".*/\1=\2/p')
t1b=$(now)
[ -z "$tid" ] && fail=$((fail+1))
echo "1 seal_inverse [$(el $t1a $t1b)s]: inverse S^-1 sealed for transformation=${tid:-<NONE>} policy=${pol:-<NONE>}"

# --- stage 2: mutation (commit) ---
t2a=$(now)
o=$($GX --project "$proj" commit "$tid" 2>"$root/e4"); rc=$?
crcpt=$(printf '%s' "$o" | sed -n 's/.*"stored_at":"\([^"]*\)".*/\1/p'|head -1)
t2b=$(now)
after_sha=$(sha256sum "$target" | cut -d' ' -f1)
head_after=$(git -C "$root/clone" rev-parse HEAD)
[ $rc -ne 0 ] && fail=$((fail+1))
ok=no; [ "$head_after" != "$head" ] && [ "$after_sha" = "$before_sha" ] && ok=yes
[ "$ok" != yes ] && fail=$((fail+1))
echo "2 mutation(commit) [$(el $t2a $t2b)s policy=$pol]: HEAD ${head:0:7}->${head_after:0:7} worktree_bytes_unchanged(index/HEAD only) status=$([ "$ok" = yes ] && echo OK || echo FAIL)"

# --- stage 3: receipt (checkpoint + online verify) ---
t3a=$(now)
$GX --project "$proj" log checkpoint --key "$key_file" --out "$root/head1.json" >/dev/null 2>"$root/e6"; rc1=$?
o=$($GX --project "$proj" receipt verify "$crcpt" --key "$root/pub.json" --checkpoint "$root/head1.json" --checkpoint-key "$root/pub.json" 2>"$root/e7"); rc2=$?
valid=$(printf '%s' "$o" | grep -o '"valid":true' | head -1)
t3b=$(now)
{ [ $rc1 -ne 0 ] || [ $rc2 -ne 0 ] || [ -z "$valid" ]; } && fail=$((fail+1))
echo "3 receipt(checkpoint+verify) [$(el $t3a $t3b)s]: commit receipt against merkle checkpoint -> $([ -n "$valid" ] && echo valid || echo INVALID)"

# --- stage 4: byte tamper -- on a COPY only. The original commit receipt is left untouched,
# because stage 6 (undo) and stage 7 (redo) need the real receipt. ---
t4a=$(now)
if [ -z "$crcpt" ] || [ ! -f "$crcpt" ]; then
  echo "4 byte_tamper [0.000s]: SKIPPED -- commit receipt path not located (crcpt='$crcpt')"
  fail=$((fail+1))
else
  cp "$crcpt" "$root/receipt_tampered.json"
  python3 - "$root/receipt_tampered.json" <<'PY'
import json,sys
p=sys.argv[1]
d=json.load(open(p))
pl=d['envelope']['payload']
d['envelope']['payload']=pl[:20]+('A' if pl[20]!='A' else 'B')+pl[21:]
json.dump(d,open(p,'w'))
PY
  t4b=$(now)
  diff_ok=no; cmp -s "$crcpt" "$root/receipt_tampered.json" || diff_ok=yes
  [ "$diff_ok" != yes ] && fail=$((fail+1))
  echo "4 byte_tamper [$(el $t4a $t4b)s]: 1 payload byte flipped on COPY(receipt_tampered.json); original commit receipt untouched bytes_actually_differ=$diff_ok"
fi

# --- stage 5: verification failure -- verify the tampered COPY. Polarity is inverted vs stage 3
# (rejection == success here), so this uses a separate variable name (ok_reject) instead of
# reusing "ok", to avoid silently flipping what that name means. ---
t5a=$(now)
o=$($GX --project "$proj" receipt verify "$root/receipt_tampered.json" --key "$root/pub.json" --checkpoint "$root/head1.json" --checkpoint-key "$root/pub.json" 2>"$root/e_tamper"); rc_tamper=$?
valid_tamper=$(printf '%s' "$o" | grep -o '"valid":true' | head -1)
t5b=$(now)
ok_reject=no; { [ $rc_tamper -ne 0 ] || [ -z "$valid_tamper" ]; } && ok_reject=yes
[ "$ok_reject" != yes ] && fail=$((fail+1))
echo "5 verification_failure [$(el $t5a $t5b)s]: verify(tampered_copy) rc=$rc_tamper valid=$([ -n "$valid_tamper" ] && echo true || echo false) -> rejected_as_expected=$ok_reject"

# --- stage 6: undo -- on the ORIGINAL transformation/receipt. Also captures T_u, this undo's own
# transformation id, from the "transformation" field of its output (not "id"). ---
t6a=$(now)
o=$($GX --project "$proj" undo "$tid" 2>"$root/e8"); rc=$?
urcpt=$(printf '%s' "$o" | sed -n 's/.*"stored_at":"\([^"]*\)".*/\1/p'|head -1)
tu=$(printf '%s' "$o" | sed -n 's/.*"transformation":"\(gx1:[^"]*\)".*/\1/p'|head -1)
t6b=$(now)
rest_sha=$(sha256sum "$target" | cut -d' ' -f1)
head_undo=$(git -C "$root/clone" rev-parse HEAD)
ok=no; [ "$rest_sha" = "$before_sha" ] && [ "$head_undo" = "$head" ] && ok=yes
[ $rc -ne 0 ] && fail=$((fail+1))
[ "$ok" != yes ] && fail=$((fail+1))
[ -z "$tu" ] && fail=$((fail+1))
echo "6 undo [$(el $t6a $t6b)s]: HEAD restored ${head_undo:0:7}(orig ${head:0:7}) T_u=${tu:-<NONE>} status=$([ "$ok" = yes ] && echo OK || echo FAIL)"
# a checkpoint taken right after undo's own insertion -- append-only log inclusion proofs are
# checked against a checkpoint at/near the entry's own tree_size, not a later one.
$GX --project "$proj" log checkpoint --key "$key_file" --out "$root/head_b.json" >/dev/null 2>"$root/e_ckpt_b"

# --- stage 7: redo = gx undo <T_u> -- undoing the undo brings the mutation back. This is a
# normal write through the same commit/undo path, not a special "redo" verb. ---
t7a=$(now)
if [ -z "$tu" ]; then
  echo "7 redo(gx_undo_\$T_u) [0.000s]: SKIPPED -- T_u not captured in stage6 (see e8/stage6 output)"
  fail=$((fail+1))
else
  o=$($GX --project "$proj" undo "$tu" 2>"$root/e9"); rc=$?
  rrcpt=$(printf '%s' "$o" | sed -n 's/.*"stored_at":"\([^"]*\)".*/\1/p'|head -1)
  t7b=$(now)
  redo_sha=$(sha256sum "$target" | cut -d' ' -f1)
  head_redo=$(git -C "$root/clone" rev-parse HEAD)
  ok=no; [ "$redo_sha" = "$after_sha" ] && [ "$head_redo" = "$head_after" ] && ok=yes
  [ $rc -ne 0 ] && fail=$((fail+1))
  [ "$ok" != yes ] && fail=$((fail+1))
  echo "7 redo(gx_undo_\$T_u) [$(el $t7a $t7b)s]: HEAD ${head_redo:0:7}(post-mutation ${head_after:0:7}) status=$([ "$ok" = yes ] && echo OK || echo FAIL)"
  $GX --project "$proj" log checkpoint --key "$key_file" --out "$root/head_c.json" >/dev/null 2>"$root/e_ckpt_c"
fi
echo "note: repeated undo on the same intent is a known edge case (not found on the second attempt) -- this run is linear (single commit -> undo -> redo, no branch) so it does not come up here."

# --- closing: verify the commit/undo/redo receipts under the offline gate (a separate process,
# no network) -- the redo receipt must pass the same gate the first one did. Each receipt is
# checked against a checkpoint taken at/near its OWN insertion (head1=after commit, head_b=after
# undo, head_c=after redo) -- checking all three against one later, shared checkpoint does not
# work (the log's inclusion proof rejects a checkpoint taken well past the entry's own tree
# size). ---
t8a=$(now)
for lbl_p in "commit:$crcpt:$root/head1.json" "undo:$urcpt:$root/head_b.json" "redo:${rrcpt:-}:$root/head_c.json"; do
  lbl=${lbl_p%%:*}; rest=${lbl_p#*:}; p=${rest%%:*}; ckpt=${rest#*:}
  if [ -z "$p" ] || [ ! -f "$p" ] || [ ! -f "$ckpt" ]; then echo "  offline_gate $lbl: receipt or checkpoint not located (receipt=$p checkpoint=$ckpt)"; fail=$((fail+1)); continue; fi
  o2=$($GX receipt verify "$p" --offline --checkpoint "$ckpt" --checkpoint-key "$root/pub.json" --key "$root/pub.json" 2>"$root/e_off_$lbl"); rc2=$?
  v2=$(printf '%s' "$o2" | grep -o '"valid":true' | head -1)
  { [ "$rc2" != 0 ] || [ -z "$v2" ]; } && fail=$((fail+1))
  echo "  offline_gate $lbl: rc=$rc2 valid=$([ -n "$v2" ] && echo true || echo false) checkpoint=$(basename "$ckpt")"
done
t8b=$(now)
echo "closing offline_gate(3 receipts, each against its own-time checkpoint) [$(el $t8a $t8b)s]"

t_end=$(now)
echo "T_total=$(el $t_start $t_end)"
echo "FAILURES=$fail"
echo "ONE_SCREEN_RESULT=$([ $fail -eq 0 ] && echo PASS || echo FAIL)"
exit $fail
