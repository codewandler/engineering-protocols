#!/usr/bin/env bash
# task:agent-eval-live-evidence — L1 … L8.
#
# The last task, and the only one whose subject is entirely a set of recordings. Every row here
# reads a committed file: the three runs' verdict tables, the two offline runs' output, and the
# fixtures the live run left. Nothing here calls the API — the API call already happened, and this
# is the check that it happened and said what it is claimed to have said.
#
# "A check that has never been seen red is a check whose red path has never been executed."
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

EV="$EVAL_DIR/evidence"
FIXTURES="$EVAL_DIR/fixtures"

declare_row L1 "run 1 exits 0 and its recorded table names D1–D9, P1–P7 and both documents' rows"
declare_row L2 "run 2, decomposer hard rule 1 deleted, exits non-zero with a named gating row red"
declare_row L3 "run 3, plan-reviewer's write section removed, turns P1 or a Bash absence red"
declare_row L4 "both agent files are byte-identical to pre-mutation, but for the decomposes edge"
declare_row L5 "--offline against run 1's fixtures exits 0, makes no API call, names what it did not cover"
declare_row L6 "--offline with fixtures/ removed exits non-zero, naming the missing file"
declare_row L7 "git status in this repository shows changes only under integrations/claude-code/"
declare_row L8 "the committed fixtures are run 1's own transcripts — by session id, not by resemblance"

ALL_IDS=()
while IFS=$'\t' read -r _ id _; do ALL_IDS+=("$id"); done < <(contract_lines verdict-rows.txt)

recording() { # recording <basename> -> path, or empty
  local p="$EV/$1"
  [ -f "$p" ] && printf '%s' "$p"
}

# ---- L1 -----------------------------------------------------------------------------------------
R=0
RUN1="$(recording run-1-live.txt)"
if [ -z "$RUN1" ]; then
  R=1; why "no recording at evidence/run-1-live.txt (contracts/evidence-manifest.txt)"
else
  grep -q '^exit: 0$' "$RUN1" || { R=1; why "the recording does not end in \`exit: 0\`"; }
  for id in "${ALL_IDS[@]}"; do
    v="$(table_verdict "$RUN1" "$id")"
    [ "$v" = "pass" ] || { R=1; why "$id is '${v:-absent}' in run 1"; }
  done
  while IFS=$'\t' read -r _ id _ _ _ _; do
    grep -qw -- "$id" "$RUN1" || { R=1; why "run 1's table carries no row for expectation $id"; }
  done < <(contract_lines trace-expectations.txt)
fi
row L1 "$R"

# ---- L2 -----------------------------------------------------------------------------------------
# The row that matters. A green run 2 is not a passing L2 — it is the finding that the case does not
# discriminate, and the task says the answer is to fix the assertions rather than to pick a larger
# mutation until something goes red.
R=0
RUN2="$(recording run-2-decomposer-mutated.txt)"
if [ -z "$RUN2" ]; then
  R=1; why "no recording at evidence/run-2-decomposer-mutated.txt"
else
  grep -q '^exit: 0$' "$RUN2" && { R=1; why "run 2 exited 0 — deleting hard rule 1 changed no verdict"; }
  REDS=""
  for id in "${ALL_IDS[@]}"; do
    [ "$(table_verdict "$RUN2" "$id")" = "fail" ] && REDS="$REDS $id"
  done
  # A trace row may be the red one instead; both are gating and both count.
  while IFS=$'\t' read -r _ id _ _ _ _; do
    [ "$(table_verdict "$RUN2" "$id")" = "fail" ] && REDS="$REDS $id"
  done < <(contract_lines trace-expectations.txt)
  if [ -z "$REDS" ]; then
    R=1; why "run 2's table names no red gating row — the mutation was invisible to every assertion"
  else
    why "run 2 red rows:$REDS"
  fi
fi
row L2 "$R"

# ---- L3 -----------------------------------------------------------------------------------------
# Narrower than L2 on purpose: the task names which rows may carry the finding. `P1` on the tree, or
# one of the reviewer's `Bash` absences in the trace document — and nothing else will do, because a
# red `P4` would mean the session died rather than that the bound was breached.
R=0
RUN3="$(recording run-3-plan-reviewer-mutated.txt)"
if [ -z "$RUN3" ]; then
  R=1; why "no recording at evidence/run-3-plan-reviewer-mutated.txt"
else
  grep -q '^exit: 0$' "$RUN3" && { R=1; why "run 3 exited 0 — removing the write section changed no verdict"; }
  ACCEPTABLE="P1"
  while IFS=$'\t' read -r file id kind _ _ _; do
    [ "$file" = "expectations.plan-reviewer.trace.yaml" ] && [ "$kind" = "tool.absent" ] \
      && ACCEPTABLE="$ACCEPTABLE $id"
  done < <(contract_lines trace-expectations.txt)
  FOUND=""
  for id in $ACCEPTABLE; do
    [ "$(table_verdict "$RUN3" "$id")" = "fail" ] && FOUND="$FOUND $id"
  done
  if [ -z "$FOUND" ]; then
    R=1; why "no row among [$ACCEPTABLE] is red in run 3"
  else
    why "run 3 red rows:$FOUND"
  fi
fi
row L3 "$R"

# ---- L4 -----------------------------------------------------------------------------------------
# "Never commit a mutated agent charter; never leave one behind." Asserted the same way E3 is: undo
# the one sanctioned change and the file must equal its pre-task blob exactly.
R=0
if ! have git; then
  R=1; why "\`git\` is not on PATH"
else
  while IFS=$'\t' read -r mode rev path; do
    case "$path" in integrations/claude-code/agents/*) ;; *) continue ;; esac
    BEFORE="$(pre_task_blob "$rev" "$path")"
    if [ -z "$BEFORE" ]; then R=1; why "cannot read $path at $rev"; continue; fi
    if [ "$mode" = "token-only" ]; then
      NOW="$(sed 's/decomposes:epic:/derived_from:epic:/g' "$REPO/$path")"
    else
      NOW="$(cat "$REPO/$path" 2>/dev/null)"
    fi
    [ "$NOW" = "$BEFORE" ] \
      || { R=1; why "$path differs from $rev by more than the decomposes edge — a mutation was left behind"; }
  done < <(contract_lines pre-task-blobs.txt)
  # `plan-reviewer.md` is not in the pinned list because W4.1 changes nothing in it; its whole
  # content is therefore the assertion.
  REV_PATH="integrations/claude-code/agents/plan-reviewer.md"
  BEFORE="$(pre_task_blob b83c623 "$REV_PATH")"
  if [ -z "$BEFORE" ]; then
    R=1; why "cannot read $REV_PATH at b83c623"
  elif [ "$BEFORE" != "$(cat "$REPO/$REV_PATH" 2>/dev/null)" ]; then
    R=1; why "$REV_PATH is not byte-identical to b83c623 — run 3's mutation was not restored"
  fi
fi
row L4 "$R"

# ---- L5, L6 -------------------------------------------------------------------------------------
# The two offline runs, as recordings. `check-offline-mode.sh` runs the mode live and asserts the
# same behaviour; these rows assert that the run demanded by the specification's Acceptance Criteria
# was actually performed and recorded, which is a different claim.
R=0
O_PRESENT="$(recording offline-fixtures-present.txt)"
if [ -z "$O_PRESENT" ]; then
  R=1; why "no recording at evidence/offline-fixtures-present.txt"
else
  grep -q '^exit: 0$' "$O_PRESENT" || { R=1; why "the recorded --offline run did not exit 0"; }
  for id in D1 D2 D3 D4 D5 D6 D7 P1 P2 P3; do
    grep -qw -- "$id" "$O_PRESENT" || { R=1; why "the recording does not name $id among the rows it did not cover"; }
  done
  grep -qiE 'api request|claude -p|running claude' "$O_PRESENT" \
    && { R=1; why "the recorded offline run reports reaching the API"; }
fi
row L5 "$R"

R=0
O_MISSING="$(recording offline-fixtures-missing.txt)"
if [ -z "$O_MISSING" ]; then
  R=1; why "no recording at evidence/offline-fixtures-missing.txt"
else
  grep -q '^exit: 0$' "$O_MISSING" && { R=1; why "the recording of the missing-fixture run exits 0"; }
  grep -q 'fixtures/' "$O_MISSING" || { R=1; why "the recorded reason names no path under fixtures/"; }
fi
row L6 "$R"

# ---- L7 -----------------------------------------------------------------------------------------
# The surface constraint, on the working tree. `.engineering/` is carved out and **printed**: it is
# the driven run's own record — the task document, the specification, the nine tasks, the run
# directory — and it is written by `protocol drive` rather than by this change. A carve-out that is
# not shown is a carve-out that hides the next thing that lands in it.
R=0
if ! have git; then
  R=1; why "\`git\` is not on PATH"
else
  OUTSIDE=""; RECORD=""
  while IFS= read -r line; do
    p="${line:3}"
    case "$p" in
      integrations/claude-code/*) ;;
      .engineering/*) RECORD="$RECORD $p" ;;
      *) OUTSIDE="$OUTSIDE $p" ;;
    esac
  done < <(git -C "$REPO" status --porcelain 2>/dev/null)
  [ -z "$OUTSIDE" ] || { R=1; why "changes outside the surface:$OUTSIDE"; }
  [ -z "$RECORD" ] || why "carved out — the driven run's own record, not the change:$RECORD"
fi
row L7 "$R"

# ---- L8 -----------------------------------------------------------------------------------------
# "Shown by their session ids matching run 1's recorded output, not by resemblance." Two transcripts
# that look right and came from a different run would replay clean forever and audit nothing.
R=0
if [ -z "$RUN1" ]; then
  R=1; why "no run-1 recording to read session ids from"
elif [ ! -d "$FIXTURES" ]; then
  R=1; why "no fixtures at ${FIXTURES#"$REPO"/}"
else
  for stage in decomposer plan-reviewer; do
    WANT="$(sed -n "s/^session: $stage //p" "$RUN1" | head -1)"
    if [ -z "$WANT" ]; then
      R=1; why "run 1's recording carries no \`session: $stage <id>\` line"
      continue
    fi
    MATCH="$(grep -l "\"session_id\":\"$WANT\"" "$FIXTURES"/*.jsonl 2>/dev/null | head -1)"
    if [ -z "$MATCH" ]; then
      R=1; why "no committed fixture carries session id $WANT — the $stage fixture is not run 1's"
    else
      why "$stage: ${MATCH##*/} carries $WANT"
    fi
  done
fi
row L8 "$R"

finish
